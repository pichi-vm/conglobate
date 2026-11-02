// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::path::{FilePath, RelativePath};
use rustix::fs::{Mode, OFlags, ResolveFlags};
#[cfg(test)]
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Read},
    os::unix::fs::PermissionsExt,
    path::Path,
};

/// Measured bytes of one ordinary runtime file. Symlinks are resolved during
/// capture; the captured runtime materializes ordinary files at requested paths.
/// Set-ID bits are deliberately not part of the trusted runtime permissions.
#[derive(Debug)]
pub(crate) struct FormatterFile {
    bytes: Box<[u8]>,
    #[cfg(test)]
    digest: [u8; 32],
    executable: bool,
}
impl FormatterFile {
    #[cfg(test)]
    pub(crate) fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub(crate) fn executable(&self) -> bool {
        self.executable
    }
    fn capture(root: &File, path: &RelativePath<FilePath>) -> io::Result<Self> {
        let mut file = File::from(rustix::fs::openat2(
            root,
            path.as_ref(),
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
            ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS,
        )?);
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "formatter input must be a regular file",
            ));
        }
        let limit = metadata
            .len()
            .checked_add(1)
            .ok_or_else(|| io::Error::other("formatter input size overflow"))?;
        let mut bytes = Vec::new();
        (&mut file).take(limit).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != metadata.len() {
            return Err(io::Error::other(
                "formatter input size changed during capture",
            ));
        }
        Ok(Self {
            #[cfg(test)]
            digest: Sha256::digest(&bytes).into(),
            bytes: bytes.into_boxed_slice(),
            executable: metadata.permissions().mode() & 0o111 != 0,
        })
    }
}

/// Immutable captured executable and runtime dependencies supplied by the trusted
/// environment. No host paths or open source descriptors survive capture. This
/// type is measured input, not evidence that formatting ran successfully.
///
/// The environment lists the loader and required libraries at their runtime
/// paths. Missing dependencies must fail inside the eventual captured root;
/// execution must never fall back to the original host filesystem.
#[derive(Debug)]
pub(crate) struct CapturedFormatter {
    executable: RelativePath<FilePath>,
    files: BTreeMap<RelativePath<FilePath>, FormatterFile>,
    fat: Option<RelativePath<FilePath>>,
}
impl CapturedFormatter {
    pub(super) fn with_fat(mut self, fat: Self) -> io::Result<Self> {
        for (path, file) in &fat.files {
            if self.files.iter().any(|(existing, old)| {
                if existing == path {
                    old.bytes != file.bytes || old.executable != file.executable
                } else {
                    existing.as_ref().starts_with(path.as_ref())
                        || path.as_ref().starts_with(existing.as_ref())
                }
            }) {
                return Err(io::Error::other(
                    "trusted formatter inputs conflict between captures",
                ));
            }
        }
        self.fat = Some(fat.executable);
        self.files.extend(fat.files);
        Ok(self)
    }
    pub(super) fn fat(&self) -> Option<&RelativePath<FilePath>> {
        self.fat.as_ref()
    }
    pub(crate) fn capture(
        root: &Path,
        executable: RelativePath<FilePath>,
        paths: impl IntoIterator<Item = RelativePath<FilePath>>,
    ) -> io::Result<Self> {
        let root = File::open(root)?;
        if !root.metadata()?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "formatter source root must be a directory",
            ));
        }
        let mut files = BTreeMap::new();
        for path in paths {
            if files.contains_key(&path) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "duplicate formatter runtime path",
                ));
            }
            let file = FormatterFile::capture(&root, &path)?;
            files.insert(path, file);
        }
        for path in files.keys() {
            for ancestor in path
                .as_ref()
                .ancestors()
                .skip(1)
                .filter(|p| !p.as_os_str().is_empty())
            {
                if files.keys().any(|file| file.as_ref() == ancestor) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "formatter runtime file conflicts with a directory",
                    ));
                }
            }
        }
        if !files
            .get(&executable)
            .is_some_and(FormatterFile::executable)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "formatter executable must be captured with execute permission",
            ));
        }
        Ok(Self {
            executable,
            files,
            fat: None,
        })
    }
    /// Add the builder-owned configuration, rejecting a supplied host config.
    /// The captured root must never choose ambient mke2fs defaults.
    pub(crate) fn with_ext4_configuration(mut self) -> io::Result<Self> {
        let path = RelativePath::try_from(Path::new(super::Ext4Format::CONFIGURATION_PATH))
            .expect("fixed configuration path");
        if self.files.keys().any(|existing| {
            path.as_ref().starts_with(existing.as_ref())
                || existing.as_ref().starts_with(path.as_ref())
        }) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "captured runtime conflicts with builder-owned formatter configuration",
            ));
        }
        self.files.insert(
            path,
            FormatterFile {
                bytes: super::Ext4Format::configuration().into(),
                #[cfg(test)]
                digest: super::Ext4Format::configuration_digest(),
                executable: false,
            },
        );
        Ok(self)
    }
    pub(crate) fn executable(&self) -> &RelativePath<FilePath> {
        &self.executable
    }
    pub(crate) fn files(
        &self,
    ) -> impl ExactSizeIterator<Item = (&RelativePath<FilePath>, &FormatterFile)> {
        self.files.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn configuration_is_builder_owned_and_rejects_runtime_path_collisions() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("tool"), b"tool").unwrap();
        fs::set_permissions(root.path().join("tool"), fs::Permissions::from_mode(0o755)).unwrap();
        let tool = RelativePath::try_from(Path::new("tool")).unwrap();
        let captured = CapturedFormatter::capture(root.path(), tool.clone(), [tool.clone()])
            .unwrap()
            .with_ext4_configuration()
            .unwrap();
        let (_, config) = captured
            .files()
            .find(|(path, _)| {
                path.as_ref() == Path::new(super::super::Ext4Format::CONFIGURATION_PATH)
            })
            .unwrap();
        assert_eq!(config.bytes(), super::super::Ext4Format::configuration());
        assert!(!config.executable());
        assert!(captured.with_ext4_configuration().is_err());

        for collision in ["etc", "etc/mke2fs.conf", "etc/mke2fs.conf/child"] {
            let other = tempfile::tempdir().unwrap();
            fs::copy(root.path().join("tool"), other.path().join("tool")).unwrap();
            let target = other.path().join(collision);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, b"ambient configuration").unwrap();
            let path = RelativePath::try_from(Path::new(collision)).unwrap();
            let captured =
                CapturedFormatter::capture(other.path(), tool.clone(), [tool.clone(), path])
                    .unwrap();
            assert!(captured.with_ext4_configuration().is_err(), "{collision}");
        }
    }

    #[test]
    fn capture_resolves_in_root_and_retains_measured_bytes_after_source_change() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        fs::write(root.path().join("bin/tool"), b"captured executable").unwrap();
        fs::set_permissions(
            root.path().join("bin/tool"),
            fs::Permissions::from_mode(0o4755),
        )
        .unwrap();
        symlink("/bin/tool", root.path().join("formatter")).unwrap();
        let path = RelativePath::try_from(Path::new("formatter")).unwrap();
        let captured = CapturedFormatter::capture(root.path(), path.clone(), [path]).unwrap();
        fs::write(root.path().join("bin/tool"), b"changed").unwrap();
        let (_, file) = captured.files().next().unwrap();
        assert_eq!(file.bytes(), b"captured executable");
        assert_eq!(
            file.digest(),
            &<[u8; 32]>::from(Sha256::digest(file.bytes()))
        );
        assert!(file.executable());
    }

    #[test]
    fn runtime_rejects_duplicates_missing_files_and_nonexecutables() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("tool"), b"tool").unwrap();
        let path = RelativePath::try_from(Path::new("tool")).unwrap();
        assert!(CapturedFormatter::capture(root.path(), path.clone(), [path.clone()]).is_err());
        fs::set_permissions(root.path().join("tool"), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            CapturedFormatter::capture(root.path(), path.clone(), [path.clone(), path.clone()])
                .is_err()
        );
        let missing = RelativePath::try_from(Path::new("missing")).unwrap();
        assert!(
            CapturedFormatter::capture(root.path(), path.clone(), [path.clone(), missing]).is_err()
        );
        assert!(CapturedFormatter::capture(root.path(), path, []).is_err());
    }
}
