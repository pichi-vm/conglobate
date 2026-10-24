// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::CapturedFormatter;
use crate::storage::Workspace;
use rustix::mount::{
    MountAttr, MountAttrFlags, MountPropagationFlags, MountSetAttrFlags, OpenTreeFlags,
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
    fn close(self) -> io::Result<()> {
        self.directory.close()
    }
}
/// Private executable read-only mount containing exactly captured ordinary files.
/// The detached root closes before backing deletion. No mutable backing handle
/// escapes; successful staging is not a formatter execution receipt.
#[derive(Debug)]
pub(crate) struct FormatterRuntime {
    pub(super) root: File,
    backing: Backing,
    captured: CapturedFormatter,
}
impl CapturedFormatter {
    pub(crate) fn stage(self, workspace: &Workspace) -> anyhow::Result<FormatterRuntime> {
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
        })()?;
        let staged = (|| -> io::Result<File> {
            if self
                .files()
                .any(|(path, _)| path.as_ref().starts_with("dev"))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "formatter runtime reserves /dev for its assigned device",
                ));
            }
            fs::create_dir(backing.directory.path().join("dev"))?;
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
            Err(source) => {
                let error = anyhow::Error::new(source);
                Err(match backing.close() {
                    Ok(()) => error,
                    Err(cleanup) => error.context(format!("formatter staging cleanup: {cleanup}")),
                })
            }
        }
    }
}
impl FormatterRuntime {
    pub(crate) fn captured(&self) -> &CapturedFormatter {
        &self.captured
    }
    pub(crate) fn close(self) -> io::Result<()> {
        let Self {
            root,
            backing,
            captured: _,
        } = self;
        drop(root);
        backing.close()
    }
}
