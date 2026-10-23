// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::CapturedFormatter;
use crate::{
    path::{FilePath, RelativePath},
    storage::Workspace,
};
use rustix::{
    fs::{Mode, OFlags, ResolveFlags},
    mount::{MountAttr, MountAttrFlags, MountPropagationFlags, MountSetAttrFlags, OpenTreeFlags},
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
};

#[derive(Debug)]
struct Backing {
    directory: tempfile::TempDir,
    _parent: File,
}
impl Backing {
    fn close(self) -> Result<(), FormatterCleanupFailure> {
        if let Err(source) = fs::remove_dir_all(self.directory.path())
            && source.kind() != io::ErrorKind::NotFound
        {
            return Err(FormatterCleanupFailure {
                source,
                backing: self,
            });
        }
        let Self { directory, _parent } = self;
        let _ = directory.keep();
        drop(_parent);
        Ok(())
    }
}
#[derive(Debug, thiserror::Error)]
#[error("formatter runtime cleanup: {source}")]
pub struct FormatterCleanupFailure {
    source: io::Error,
    backing: Backing,
}
impl FormatterCleanupFailure {
    pub fn source_error(&self) -> &io::Error {
        &self.source
    }
    pub fn retry(self) -> Result<(), Self> {
        self.backing.close()
    }
}
#[derive(Debug, thiserror::Error)]
#[error("formatter staging: {source}")]
pub struct FormatterStageFailure {
    source: io::Error,
    backing: Option<Backing>,
}
impl FormatterStageFailure {
    pub fn source_error(&self) -> &io::Error {
        &self.source
    }
    pub fn cleanup(self) -> Result<(), FormatterCleanupFailure> {
        match self.backing {
            Some(backing) => backing.close(),
            None => Ok(()),
        }
    }
}

/// Private executable read-only mount containing exactly captured ordinary files.
/// The detached root closes before backing deletion. No mutable backing handle
/// escapes; successful staging is not a formatter execution receipt.
#[derive(Debug)]
pub struct FormatterRuntime {
    root: File,
    backing: Backing,
    captured: CapturedFormatter,
}
impl CapturedFormatter {
    pub fn stage(self, workspace: &Workspace) -> Result<FormatterRuntime, FormatterStageFailure> {
        let backing = (|| -> io::Result<Backing> {
            let parent = workspace.staging_parent()?;
            let directory = tempfile::Builder::new()
                .prefix("conglobate-formatter-")
                .permissions(fs::Permissions::from_mode(0o700))
                .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))?;
            Ok(Backing {
                directory,
                _parent: parent,
            })
        })()
        .map_err(|source| FormatterStageFailure {
            source,
            backing: None,
        })?;
        let staged = (|| -> io::Result<File> {
            for (path, input) in self.files() {
                let target = backing.directory.path().join(path.as_ref());
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut output = OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .mode(0o600)
                    .open(target)?;
                output.write_all(input.bytes())?;
                output.set_permissions(fs::Permissions::from_mode(if input.executable() {
                    0o555
                } else {
                    0o444
                }))?;
                output.sync_all()?;
            }
            let root = File::open(backing.directory.path())?;
            let tree = rustix::mount::open_tree(
                &root,
                "",
                OpenTreeFlags::AT_EMPTY_PATH
                    | OpenTreeFlags::OPEN_TREE_CLONE
                    | OpenTreeFlags::OPEN_TREE_CLOEXEC,
            )?;
            rustix::mount::mount_setattr(
                &tree,
                "",
                MountSetAttrFlags::AT_EMPTY_PATH,
                MountAttr::new(
                    MountAttrFlags::MOUNT_ATTR_RDONLY
                        | MountAttrFlags::MOUNT_ATTR_NOSUID
                        | MountAttrFlags::MOUNT_ATTR_NODEV,
                    MountAttrFlags::MOUNT_ATTR_NOEXEC,
                    MountPropagationFlags::PRIVATE,
                ),
            )?;
            Ok(tree.into())
        })();
        match staged {
            Ok(root) => Ok(FormatterRuntime {
                root,
                backing,
                captured: self,
            }),
            Err(source) => Err(FormatterStageFailure {
                source,
                backing: Some(backing),
            }),
        }
    }
}
impl FormatterRuntime {
    pub fn captured(&self) -> &CapturedFormatter {
        &self.captured
    }
    /// Open measured runtime content through the read-only view. Returned files
    /// cannot regain the writable backing mount, including via chmod/truncate.
    pub fn open(&self, path: &RelativePath<FilePath>) -> io::Result<File> {
        Ok(rustix::fs::openat2(
            &self.root,
            path.as_ref(),
            OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )?
        .into())
    }
    pub fn close(self) -> Result<(), FormatterCleanupFailure> {
        let Self {
            root,
            backing,
            captured: _,
        } = self;
        drop(root);
        backing.close()
    }
}
