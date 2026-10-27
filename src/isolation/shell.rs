// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Private `/bin/sh` for a writable root that does not have one.
//!
//! Carapace `run` pivots into the image and execs `/bin/sh`. The context-recipe
//! smoke's source carapace is a tiny ext4 (`os-release` only), so that exec
//! cannot succeed even after `/dev`, `/proc`, and `/sys` exist. When `bin/sh`
//! is already a regular file this module does nothing. Otherwise it mounts a
//! private tmpfs over every top-level directory the closure touches, whether
//! or not that directory already exists, and copies the appliance shell,
//! `cat`, and their dynamic-loader closure there. The clone used for
//! `pivot_root` includes those mounts; they are not part of the sealed layer.
//! Only the empty mount-point directories remain on the writable snapshot.
//! A symlink or other non-regular `bin/sh` or `bin/cat` fails closed.

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
const TMPFS_SIZE: &str = "64m";
const REQUIRED_TOOLS: [&str; 2] = ["bin/sh", "bin/cat"];

pub(super) fn install(image: &File) -> io::Result<()> {
    if shell_present(image)? {
        return Ok(());
    }
    for required in REQUIRED_TOOLS {
        reject_non_regular(image, required)?;
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
        Placed::required(PathBuf::from("bin/sh"), read_loader(&shell)?),
        Placed::required(PathBuf::from("bin/cat"), read_loader(&cat)?),
    ];
    for library in libs {
        let relative = relative_from_absolute(&library)?;
        if files.iter().any(|placed| placed.relative == relative) {
            continue;
        }
        files.push(Placed::loader(relative, read_loader(&library)?));
    }
    Layout::new(image).place_all(&files)
}

fn reject_non_regular(image: &File, relative: &str) -> io::Result<()> {
    match rustix::fs::openat2(
        image,
        relative,
        OFlags::PATH | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_SYMLINKS,
    ) {
        Err(rustix::io::Errno::NOENT) => Ok(()),
        Err(rustix::io::Errno::LOOP) => Err(io::Error::other(format!(
            "command shell target /{relative} is a symlink"
        ))),
        Ok(fd) => {
            let metadata = File::from(fd).metadata()?;
            if metadata.is_file() {
                Ok(())
            } else {
                Err(io::Error::other(format!(
                    "command shell target /{relative} is not a regular file"
                )))
            }
        }
        Err(error) => Err(io::Error::new(
            io::Error::from(error).kind(),
            format!("command shell target /{relative}: {error}"),
        )),
    }
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
struct Placed {
    relative: PathBuf,
    bytes: Vec<u8>,
    required: bool,
}
impl Placed {
    fn required(relative: PathBuf, bytes: Vec<u8>) -> Self {
        Self {
            relative,
            bytes,
            required: true,
        }
    }
    fn loader(relative: PathBuf, bytes: Vec<u8>) -> Self {
        Self {
            relative,
            bytes,
            required: false,
        }
    }
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

    fn place_all(&mut self, files: &[Placed]) -> io::Result<()> {
        for placed in files {
            self.place(placed)?;
        }
        Ok(())
    }

    fn place(&mut self, placed: &Placed) -> io::Result<()> {
        let mut components = placed.relative.components();
        let Some(Component::Normal(top)) = components.next() else {
            return Err(io::Error::other(format!(
                "refusing shell path {}",
                placed.relative.display()
            )));
        };
        let rest: PathBuf = components.collect();
        if rest.as_os_str().is_empty() {
            return Err(io::Error::other(format!(
                "refusing shell path {}",
                placed.relative.display()
            )));
        }
        let directory = self.directory(top)?;
        write_bytes(&directory, &rest, &placed.bytes, placed.required)
    }

    /// Always cover `top` with a private tmpfs. An existing directory is
    /// hidden for this command only; its bytes, including a pre-existing
    /// empty `/bin`, stay on the layer without the appliance binaries.
    fn directory(&mut self, top: &OsStr) -> io::Result<File> {
        if let Some((_, root)) = self.private.iter().find(|(name, _)| name == top) {
            return root.try_clone();
        }
        match open_directory(self.image, top) {
            Ok(_) => {}
            Err(rustix::io::Errno::NOENT) => {
                rustix::fs::mkdirat(self.image, top, Mode::from_raw_mode(0o755))?;
                chmod_0755(self.image, top)?;
            }
            Err(error) => return Err(explain_top(top, error)),
        }
        let point = open_directory(self.image, top).map_err(|error| explain_top(top, error))?;
        let tmp = tmpfs()?;
        mount::move_mount(
            &tmp,
            "",
            &point,
            "",
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )?;
        self.private.push((top.to_owned(), tmp.try_clone()?));
        Ok(tmp)
    }
}

fn explain_top(top: &OsStr, error: rustix::io::Errno) -> io::Error {
    let path = top.to_string_lossy();
    if error == rustix::io::Errno::LOOP {
        io::Error::other(format!(
            "command shell mount point /{path} is a symlink; not following it"
        ))
    } else {
        io::Error::new(
            io::Error::from(error).kind(),
            format!("command shell mount point /{path}: {error}"),
        )
    }
}

fn chmod_0755(directory: &File, name: &OsStr) -> io::Result<()> {
    rustix::fs::chmodat(
        directory,
        name,
        Mode::from_raw_mode(0o755),
        AtFlags::empty(),
    )
    .map_err(|error| {
        io::Error::new(
            io::Error::from(error).kind(),
            format!(
                "command shell mount point {}: {error}",
                name.to_string_lossy()
            ),
        )
    })
}

fn tmpfs() -> io::Result<File> {
    let context = mount::fsopen("tmpfs", FsOpenFlags::FSOPEN_CLOEXEC)?;
    mount::fsconfig_set_string(&context, "mode", "0755")?;
    mount::fsconfig_set_string(&context, "size", TMPFS_SIZE)?;
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

fn write_bytes(directory: &File, relative: &Path, bytes: &[u8], required: bool) -> io::Result<()> {
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
                chmod_0755(&parent, name)?;
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
        // Loader paths can name the same file twice. Required interpreter
        // names must be created on this tmpfs; keeping a prior occupant
        // would report success with no usable shell.
        Err(rustix::io::Errno::EXIST) if !required => return Ok(()),
        Err(rustix::io::Errno::EXIST) => {
            return Err(io::Error::other(format!(
                "command shell target {} exists and is not a usable regular file",
                relative.display()
            )));
        }
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
    use std::{
        ffi::OsStr,
        os::unix::fs::{PermissionsExt, symlink},
    };

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
    fn symlink_mount_point_error_names_the_top_level_dir() {
        let error = explain_top(OsStr::new("bin"), rustix::io::Errno::LOOP);
        let message = error.to_string();
        assert!(
            message.contains("/bin") && message.contains("symlink"),
            "{message}"
        );
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
    fn symlinked_regular_shell_is_left_in_place() {
        let image = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(image.path().join("usr/bin")).unwrap();
        std::fs::write(image.path().join("usr/bin/sh"), b"real-shell").unwrap();
        symlink("usr/bin", image.path().join("bin")).unwrap();
        install(&File::open(image.path()).unwrap()).unwrap();
        assert_eq!(
            std::fs::read(image.path().join("usr/bin/sh")).unwrap(),
            b"real-shell"
        );
        assert!(!image.path().join("usr/bin/cat").exists());
        assert!(!image.path().join("lib").exists());
        assert!(!image.path().join("lib64").exists());
    }

    #[test]
    fn dangling_shell_symlink_fails_closed() {
        for relative in ["bin/sh", "bin/cat"] {
            let image = tempfile::tempdir().unwrap();
            std::fs::create_dir(image.path().join("bin")).unwrap();
            symlink("missing", image.path().join(relative)).unwrap();
            let error = install(&File::open(image.path()).unwrap()).unwrap_err();
            let message = error.to_string();
            assert!(
                message.contains(relative) && message.contains("symlink"),
                "{relative}: {message}"
            );
            assert!(
                std::fs::symlink_metadata(image.path().join(relative))
                    .unwrap()
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(
                std::fs::read_dir(image.path().join("bin")).unwrap().count(),
                1,
                "{relative}"
            );
            assert!(!image.path().join("lib").exists());
            assert!(!image.path().join("lib64").exists());
        }
    }

    #[test]
    fn non_file_shell_fails_closed() {
        let image = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(image.path().join("bin/sh")).unwrap();
        let error = install(&File::open(image.path()).unwrap()).unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("bin/sh") && message.contains("not a regular file"),
            "{message}"
        );
        assert!(image.path().join("bin/sh").metadata().unwrap().is_dir());
        assert!(!image.path().join("bin/cat").exists());
        assert!(!image.path().join("lib").exists());
    }

    #[test]
    fn symlink_top_level_bin_is_not_followed() {
        let image = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(image.path().join("usr/bin")).unwrap();
        symlink("usr/bin", image.path().join("bin")).unwrap();
        let error = install(&File::open(image.path()).unwrap()).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("symlink"), "{message}");
        assert!(
            std::fs::symlink_metadata(image.path().join("bin"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            std::fs::read_dir(image.path().join("usr/bin"))
                .unwrap()
                .count(),
            0
        );
        assert!(!image.path().join("lib").exists());
    }

    #[test]
    fn private_shell_runs_the_context_recipe_script() {
        if std::env::var("CONGLOBATE_SHELL_PROBE").as_deref()
            == Ok("private_shell_runs_the_context_recipe_script")
        {
            run_minimal_shell();
            return;
        }
        unshare_probe("private_shell_runs_the_context_recipe_script");
    }

    #[test]
    fn empty_bin_keeps_no_copied_files() {
        if std::env::var("CONGLOBATE_SHELL_PROBE").as_deref()
            == Ok("empty_bin_keeps_no_copied_files")
        {
            run_empty_bin();
            return;
        }
        unshare_probe("empty_bin_keeps_no_copied_files");
    }

    fn unshare_probe(test_name: &str) {
        let output = Command::new("unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "--propagation",
                "private",
                "--fork",
            ])
            .env("CONGLOBATE_SHELL_PROBE", test_name)
            .arg(std::env::current_exe().unwrap())
            .arg(test_name)
            .output()
            .expect("unshare");
        if output.status.success() {
            return;
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if unshare_itself_denied(&stderr) {
            eprintln!("skip: unprivileged mount namespace is unavailable: {stderr}");
            return;
        }
        panic!(
            "{test_name} failed: {}\nstdout:\n{}\nstderr:\n{stderr}",
            output.status,
            String::from_utf8_lossy(&output.stdout)
        );
    }

    /// Skip only when `unshare` could not create the namespace. A child that
    /// prints "Permission denied" for some other reason must still fail.
    fn unshare_itself_denied(stderr: &str) -> bool {
        stderr.lines().any(|line| {
            let line = line.to_ascii_lowercase();
            line.contains("unshare:")
                && (line.contains("permission denied") || line.contains("operation not permitted"))
        })
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
        assert!(
            image_mounts(image.path())
                .iter()
                .any(|path| path.ends_with("bin")),
            "shell mount missing"
        );
        run_context_script(image.path());
        assert_layer_has_only_empty_mount_points(image.path());
    }

    fn run_empty_bin() {
        let image = tempfile::tempdir().expect("image");
        std::fs::create_dir(image.path().join("bin")).unwrap();
        std::fs::create_dir(image.path().join("etc")).unwrap();
        std::fs::write(
            image.path().join("etc/conglobate-context-check"),
            b"committed context bytes\n",
        )
        .unwrap();
        // Restrictive umask applies only in this unshare child. Created
        // runtime and shell directories must still end as mode 0755.
        rustix::process::umask(rustix::fs::Mode::from_raw_mode(0o077));
        let root = File::open(image.path()).unwrap();
        super::super::runtime::RuntimeTargets::resolve(&root).unwrap();
        for name in ["dev", "proc", "sys"] {
            assert_mode_0755(&image.path().join(name));
        }
        install(&root).unwrap();
        let mounts = image_mounts(image.path());
        assert!(
            mounts.iter().any(|path| path.ends_with("bin")),
            "pre-existing bin was not covered: {mounts:?}"
        );
        let mut nested = false;
        for mount in &mounts {
            nested |= assert_mounted_tree_mode_0755(mount);
        }
        assert!(
            nested,
            "loader closure had no intermediate directory to chmod"
        );
        run_context_script(image.path());
        assert_layer_has_only_empty_mount_points(image.path());
        assert_eq!(
            std::fs::read_dir(image.path().join("bin")).unwrap().count(),
            0,
            "copied files remained in pre-existing bin"
        );
    }

    fn run_context_script(root: &Path) {
        let status = Command::new("/usr/sbin/chroot")
            .arg(root)
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
    }

    fn assert_layer_has_only_empty_mount_points(root: &Path) {
        for mount in image_mounts(root) {
            rustix::mount::unmount(&mount, rustix::mount::UnmountFlags::empty()).unwrap();
        }
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.file_name().is_some_and(|name| name == "etc") {
                assert!(path.join("conglobate-context-check").is_file());
                continue;
            }
            assert!(path.is_dir(), "{}", path.display());
            assert_eq!(
                std::fs::read_dir(&path).unwrap().count(),
                0,
                "loader files persisted in {}",
                path.display()
            );
            assert_mode_0755(&path);
        }
    }

    fn assert_mode_0755(path: &Path) {
        let mode = std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "{}", path.display());
    }

    /// Returns whether `path` contains a directory below its top level.
    fn assert_mounted_tree_mode_0755(path: &Path) -> bool {
        assert_mode_0755(path);
        let mut nested = false;
        for entry in std::fs::read_dir(path).unwrap() {
            let child = entry.unwrap().path();
            let metadata = std::fs::symlink_metadata(&child).unwrap();
            assert!(
                !metadata.file_type().is_symlink(),
                "copied symlink {}",
                child.display()
            );
            assert_mode_0755(&child);
            if metadata.is_dir() {
                nested = true;
                assert_mounted_tree_mode_0755(&child);
            }
        }
        nested
    }

    fn image_mounts(root: &Path) -> Vec<PathBuf> {
        let root = root.canonicalize().unwrap();
        let text = std::fs::read_to_string("/proc/self/mountinfo").unwrap();
        let mut mounts = Vec::new();
        for line in text.lines() {
            let Some(point) = line.split(' ').nth(4) else {
                continue;
            };
            let path = PathBuf::from(point);
            if path.starts_with(&root) && path != root {
                mounts.push(path);
            }
        }
        mounts.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
        mounts
    }
}
