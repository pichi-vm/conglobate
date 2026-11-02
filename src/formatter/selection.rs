// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::CapturedFormatter;
use crate::{contract::recipe::Format, path::ContextPath};
use std::{
    collections::BTreeSet,
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

impl CapturedFormatter {
    /// Resolve the internal formatter profile for a plan filesystem specification.
    ///
    /// The kind-tagged plan enum admits only qualified backends. Match it
    /// explicitly so adding a variant requires a corresponding formatter choice.
    pub(crate) fn for_filesystem(spec: &Format) -> io::Result<CapturedFormatter> {
        match spec {
            Format::Ext4(spec) => {
                let captured = Self::for_name("ext4")?;
                if let Some(size) = spec.esp {
                    super::Fat32Format::try_from(size)?;
                    captured.with_fat(CapturedFormatter::host_tool("mkfs.fat")?)
                } else {
                    Ok(captured)
                }
            }
        }
    }

    /// Map a filesystem type name to a captured formatter runtime.
    pub(crate) fn for_name(name: &str) -> io::Result<CapturedFormatter> {
        let _profile = Self::profile_for_name(name)?;
        Self::capture_host_ext4()
    }

    /// Profile identity string for a known filesystem name, without capturing tools.
    pub(crate) fn profile_for_name(name: &str) -> Result<&'static str, io::Error> {
        match name {
            "ext4" => Ok(super::Ext4Format::PROFILE),
            other => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unsupported filesystem type: {other}"),
            )),
        }
    }

    fn capture_host_ext4() -> io::Result<CapturedFormatter> {
        CapturedFormatter::host_tool("mke2fs")?.with_ext4_configuration()
    }
}

impl CapturedFormatter {
    fn host_tool(name: &str) -> io::Result<Self> {
        let executable = Self::find_tool(name)?;
        let relative = Self::strip_root(&executable)?;
        let mut files = BTreeSet::new();
        files.insert(relative.clone());
        for dependency in Self::ldd_dependencies(&executable)? {
            files.insert(Self::strip_root(&dependency)?);
        }
        let paths = files
            .into_iter()
            .map(|path| ContextPath::try_from(path.as_path()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(io::Error::other)?;
        let executable = ContextPath::try_from(relative.as_path()).map_err(io::Error::other)?;
        Self::capture(Path::new("/"), executable, paths)
    }
    fn find_tool(name: &str) -> io::Result<PathBuf> {
        let path_var = env::var_os("PATH").unwrap_or_default();
        for directory in ["/usr/sbin", "/sbin", "/usr/bin", "/bin"]
            .map(PathBuf::from)
            .into_iter()
            .chain(env::split_paths(&path_var))
        {
            let path = directory.join(name);
            if path.is_file() {
                return fs::canonicalize(path);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("trusted formatter {name} not found"),
        ))
    }
}

impl CapturedFormatter {
    fn strip_root(path: &Path) -> io::Result<PathBuf> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            fs::canonicalize(path)?
        };
        absolute
            .strip_prefix("/")
            .map(|p| p.to_path_buf())
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("formatter path is not absolute under /: {}", path.display()),
                )
            })
    }

    fn ldd_dependencies(executable: &Path) -> io::Result<Vec<PathBuf>> {
        let output = Command::new("ldd").arg(executable).output()?;
        if !output.status.success() {
            return Err(io::Error::other(format!(
                "ldd failed for {}: {}",
                executable.display(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        let text = String::from_utf8(output.stdout).map_err(io::Error::other)?;
        let mut dependencies = Vec::new();
        for line in text.lines() {
            if line.contains("not found") {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("incomplete trusted formatter runtime: {line}"),
                ));
            }
            // Match absolute paths: "lib => /lib/..." or bare "/lib64/ld-linux..."
            if let Some(path) = line.split_whitespace().find(|token| token.starts_with('/')) {
                let path = PathBuf::from(path);
                if path.is_file() {
                    // Retain the loader's runtime pathname. Canonicalizing a
                    // usrmerge symlink would leave PT_INTERP (e.g. /lib64/...) and
                    // the dynamic linker's library search paths absent in staging.
                    // CapturedFormatter resolves the source safely and materializes
                    // ordinary files at these requested runtime paths.
                    dependencies.push(path);
                }
            }
        }
        Ok(dependencies)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_filesystem_maps_to_ext4_profile() {
        assert_eq!(
            CapturedFormatter::profile_for_name("ext4").unwrap(),
            super::super::Ext4Format::PROFILE
        );
    }

    #[test]
    fn unknown_filesystem_is_rejected() {
        let err = CapturedFormatter::profile_for_name("xfs").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
        assert!(err.to_string().contains("xfs"));
        assert!(CapturedFormatter::for_name("btrfs").is_err());
    }

    #[test]
    fn captured_runtime_contains_the_executables_exact_elf_interpreter_path() {
        let executable = CapturedFormatter::find_tool("mke2fs").unwrap();
        let bytes = fs::read(&executable).unwrap();
        let elf = goblin::elf::Elf::parse(&bytes).unwrap();
        if let Some(interpreter) = elf.interpreter {
            let captured = CapturedFormatter::for_name("ext4").unwrap();
            let relative = Path::new(interpreter).strip_prefix("/").unwrap();
            assert!(
                captured
                    .files()
                    .any(|(path, file)| path.as_ref() == relative && file.executable()),
                "missing ELF interpreter {interpreter}"
            );
        }
    }

    #[test]
    fn host_ext4_capture_includes_mke2fs_and_builder_config() {
        let captured = CapturedFormatter::for_name("ext4")
            .expect("host mke2fs must be available for unit tests");
        assert!(
            captured
                .files()
                .any(|(path, _)| path.as_ref().ends_with("mke2fs")),
            "expected mke2fs among captured runtime files"
        );
        assert!(captured.files().any(|(path, _)| {
            path.as_ref() == Path::new(super::super::Ext4Format::CONFIGURATION_PATH)
        }));
    }
}
