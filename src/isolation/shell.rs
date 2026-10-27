// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Private `/bin/sh` for a writable root that does not have one.
//!
//! Carapace `run` pivots into the image and execs `/bin/sh`. The context-recipe
//! smoke's source carapace is a tiny ext4 (`os-release` only), so that exec
//! cannot succeed even after `/dev`, `/proc`, and `/sys` exist. When `bin/sh`
//! is already present this module does nothing. Otherwise it mounts a private
//! tmpfs over each missing top-level directory and copies the appliance shell,
//! `cat`, and their dynamic-loader closure there. The clone used for
//! `pivot_root` includes those mounts; they are not part of the sealed layer.
//! Only the empty mount-point directories remain on the writable snapshot.

use rustix::{
    fs::{AtFlags, Mode, OFlags, ResolveFlags},
    mount::{self, FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags},
};
use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};

const MAX_LOADER_BYTES: u64 = 32 * 1024 * 1024;

pub(super) fn install(image: &File) -> io::Result<()> {
    if shell_present(image)? {
        return Ok(());
    }
    let shell = host_tool("sh")?;
    let cat = host_tool("cat")?;
    let mut libs = BTreeSet::new();
    for binary in [&shell, &cat] {
        for library in loader_paths(binary)? {
            libs.insert(library);
        }
    }
    let mut files = vec![
        (PathBuf::from("bin/sh"), read_loader(&shell)?),
        (PathBuf::from("bin/cat"), read_loader(&cat)?),
    ];
    for library in libs {
        let relative = relative_from_absolute(&library)?;
        if files.iter().any(|(dest, _)| dest == &relative) {
            continue;
        }
        files.push((relative, read_loader(&library)?));
    }
    Layout::new(image).place_all(&files)
}

fn shell_present(image: &File) -> io::Result<bool> {
    match rustix::fs::openat2(
        image,
        "bin/sh",
        OFlags::PATH | OFlags::CLOEXEC,
        Mode::empty(),
        // Follow image symlinks (`/bin` → `usr/bin` on usrmerge) but stay
        // inside the root. Do not use NO_XDEV: a mounted `/bin` is still the
        // image's shell and must suppress injection.
        ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS,
    ) {
        Ok(fd) => Ok(File::from(fd).metadata()?.is_file()),
        Err(rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn host_tool(name: &str) -> io::Result<PathBuf> {
    for directory in ["/bin", "/usr/bin"] {
        let path = Path::new(directory).join(name);
        if path.is_file() {
            return Ok(path);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("appliance has no {name} for a root without /bin/sh"),
    ))
}

fn read_loader(path: &Path) -> io::Result<Vec<u8>> {
    let metadata = std::fs::metadata(path)?;
    if !metadata.is_file() || metadata.len() > MAX_LOADER_BYTES {
        return Err(io::Error::other(format!(
            "refusing loader file {}",
            path.display()
        )));
    }
    std::fs::read(path)
}

fn loader_paths(binary: &Path) -> io::Result<Vec<PathBuf>> {
    let ldd = host_tool("ldd")?;
    let output = Command::new(ldd)
        .arg(binary)
        .env_clear()
        .output()
        .map_err(|error| io::Error::other(format!("ldd {}: {error}", binary.display())))?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "ldd {} failed: {}",
            binary.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    parse_ldd(&String::from_utf8_lossy(&output.stdout))
        .map_err(|error| io::Error::other(format!("ldd {}: {error}", binary.display())))
}

fn parse_ldd(stdout: &str) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() || line.contains("linux-vdso") || line.contains("linux-gate") {
            continue;
        }
        let path = if let Some((_, rest)) = line.split_once("=>") {
            let rest = rest.trim();
            if rest.starts_with("not found") {
                return Err(format!("missing library in `{line}`"));
            }
            rest.split_whitespace().next().unwrap_or("")
        } else {
            line.split_whitespace().next().unwrap_or("")
        };
        if path.starts_with('/') {
            paths.push(PathBuf::from(path));
        }
    }
    Ok(paths)
}

fn relative_from_absolute(path: &Path) -> io::Result<PathBuf> {
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => relative.push(name),
            _ => {
                return Err(io::Error::other(format!(
                    "refusing loader path {}",
                    path.display()
                )));
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err(io::Error::other(format!(
            "refusing loader path {}",
            path.display()
        )));
    }
    Ok(relative)
}

#[derive(Debug)]
struct Layout<'a> {
    image: &'a File,
    private: Vec<(OsString, File)>,
}

impl<'a> Layout<'a> {
    fn new(image: &'a File) -> Self {
        Self {
            image,
            private: Vec::new(),
        }
    }

    fn place_all(&mut self, files: &[(PathBuf, Vec<u8>)]) -> io::Result<()> {
        for (relative, bytes) in files {
            self.place(relative, bytes)?;
        }
        Ok(())
    }

    fn place(&mut self, relative: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut components = relative.components();
        let Some(Component::Normal(top)) = components.next() else {
            return Err(io::Error::other(format!(
                "refusing shell path {}",
                relative.display()
            )));
        };
        let rest: PathBuf = components.collect();
        if rest.as_os_str().is_empty() {
            return Err(io::Error::other(format!(
                "refusing shell path {}",
                relative.display()
            )));
        }
        let directory = self.directory(top)?;
        write_bytes(&directory, &rest, bytes)
    }

    fn directory(&mut self, top: &OsStr) -> io::Result<File> {
        if let Some((_, root)) = self.private.iter().find(|(name, _)| name == top) {
            return root.try_clone();
        }
        match open_directory(self.image, top) {
            Ok(existing) => Ok(existing),
            Err(rustix::io::Errno::NOENT) => {
                rustix::fs::mkdirat(self.image, top, Mode::from_raw_mode(0o755))?;
                let _ = rustix::fs::chmodat(
                    self.image,
                    top,
                    Mode::from_raw_mode(0o755),
                    AtFlags::empty(),
                );
                let point = open_directory(self.image, top)?;
                let tmp = tmpfs()?;
                mount::move_mount(
                    &tmp,
                    "",
                    &point,
                    "",
                    MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH
                        | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
                )?;
                self.private.push((top.to_owned(), tmp.try_clone()?));
                Ok(tmp)
            }
            Err(error) => Err(error.into()),
        }
    }
}

fn tmpfs() -> io::Result<File> {
    let context = mount::fsopen("tmpfs", FsOpenFlags::FSOPEN_CLOEXEC)?;
    mount::fsconfig_set_string(&context, "mode", "0755")?;
    mount::fsconfig_create(&context)?;
    // Executable: this is the command interpreter. No set-id files or devices.
    Ok(mount::fsmount(
        &context,
        FsMountFlags::FSMOUNT_CLOEXEC,
        MountAttrFlags::MOUNT_ATTR_NOSUID | MountAttrFlags::MOUNT_ATTR_NODEV,
    )?
    .into())
}

fn open_directory(root: &File, path: &OsStr) -> Result<File, rustix::io::Errno> {
    Ok(rustix::fs::openat2(
        root,
        path,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
    )?
    .into())
}

fn write_bytes(directory: &File, relative: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut parent = directory.try_clone()?;
    let names: Vec<OsString> = relative
        .components()
        .map(|component| match component {
            Component::Normal(name) => Ok(name.to_owned()),
            _ => Err(io::Error::other(format!(
                "refusing shell path {}",
                relative.display()
            ))),
        })
        .collect::<io::Result<_>>()?;
    let Some((file_name, parents)) = names.split_last() else {
        return Err(io::Error::other("empty shell path"));
    };
    for name in parents {
        parent = match open_directory(&parent, name) {
            Ok(next) => next,
            Err(rustix::io::Errno::NOENT) => {
                rustix::fs::mkdirat(&parent, name, Mode::from_raw_mode(0o755))?;
                open_directory(&parent, name)?
            }
            Err(error) => return Err(error.into()),
        };
    }
    let mut file: File = match rustix::fs::openat(
        &parent,
        file_name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o755),
    ) {
        Ok(file) => file.into(),
        // The image already has this path. Keep its bytes; do not overwrite.
        Err(rustix::io::Errno::EXIST) => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    file.write_all(bytes)?;
    rustix::fs::chmodat(
        &parent,
        file_name,
        Mode::from_raw_mode(0o755),
        AtFlags::empty(),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn ldd_paths_skip_vdso_and_keep_absolute_targets() {
        let parsed = parse_ldd(
            "\tlinux-vdso.so.1 (0x1)\n\tlibc.so.6 => /lib/x86_64-linux-gnu/libc.so.6 (0x2)\n\t/lib64/ld-linux-x86-64.so.2 (0x3)\n",
        )
        .unwrap();
        assert_eq!(
            parsed,
            vec![
                PathBuf::from("/lib/x86_64-linux-gnu/libc.so.6"),
                PathBuf::from("/lib64/ld-linux-x86-64.so.2"),
            ]
        );
    }

    #[test]
    fn ldd_missing_library_is_an_error() {
        let error = parse_ldd("\tlibmissing.so.1 => not found\n").unwrap_err();
        assert!(error.contains("libmissing"), "{error}");
    }

    #[test]
    fn existing_shell_is_not_replaced() {
        let image = tempfile::tempdir().unwrap();
        std::fs::create_dir(image.path().join("bin")).unwrap();
        std::fs::write(image.path().join("bin/sh"), b"not-a-shell").unwrap();
        install(&File::open(image.path()).unwrap()).unwrap();
        assert_eq!(
            std::fs::read(image.path().join("bin/sh")).unwrap(),
            b"not-a-shell"
        );
        assert!(!image.path().join("lib").exists());
        assert!(!image.path().join("lib64").exists());
        assert!(!image.path().join("bin/cat").exists());
    }

    #[test]
    fn private_shell_runs_the_context_recipe_script() {
        if std::env::var_os("CONGLOBATE_MINIMAL_SHELL").is_some() {
            run_minimal_shell();
            return;
        }
        let output = Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "--propagation",
                "private",
                "--fork",
            ])
            .env("CONGLOBATE_MINIMAL_SHELL", "1")
            .arg(std::env::current_exe().unwrap())
            .arg("private_shell_runs_the_context_recipe_script")
            .output()
            .expect("unshare");
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("Permission denied") || stderr.contains("Operation not permitted") {
                eprintln!("skip: unprivileged mount namespace is unavailable: {stderr}");
                return;
            }
            panic!(
                "minimal root shell probe failed: {}\nstdout:\n{}\nstderr:\n{stderr}",
                output.status,
                String::from_utf8_lossy(&output.stdout)
            );
        }
    }

    fn run_minimal_shell() {
        let image = tempfile::tempdir().expect("image");
        std::fs::create_dir(image.path().join("etc")).unwrap();
        std::fs::write(
            image.path().join("etc/conglobate-context-check"),
            b"committed context bytes\n",
        )
        .unwrap();
        assert!(!image.path().join("dev").exists());
        install(&File::open(image.path()).unwrap()).unwrap();
        let mounted: Vec<_> = std::fs::read_dir(image.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .filter(|name| name != "etc")
            .collect();
        assert!(
            mounted.iter().any(|name| name == "bin"),
            "shell mount missing: {mounted:?}"
        );
        let status = Command::new("/usr/sbin/chroot")
            .arg(image.path())
            .arg("/bin/sh")
            .arg("-eu")
            .arg("-c")
            .arg(
                "test \"$(cat /etc/conglobate-context-check)\" = 'committed context bytes'\n\
                 test ! -e /context\n\
                 test ! -e /build-context\n\
                 printf 'COMMITTED_CONTEXT_COPY_PASS\\n'\n",
            )
            .env_clear()
            .env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            )
            .env("LANG", "C.UTF-8")
            .status()
            .unwrap();
        assert!(status.success(), "context recipe script failed: {status}");
        for name in &mounted {
            let path = image.path().join(name);
            rustix::mount::unmount(&path, rustix::mount::UnmountFlags::empty()).unwrap();
            assert_eq!(
                std::fs::read_dir(&path).unwrap().count(),
                0,
                "loader files persisted in {}",
                path.display()
            );
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o755,
                "{}",
                path.display()
            );
        }
    }
}
