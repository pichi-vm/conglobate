// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ContextError, ReadOnlyContext, StagedContext, StagedResources};
use crate::path::ContextPath;
use rustix::{
    fs::{Mode, OFlags, ResolveFlags},
    mount::{MountAttr, MountAttrFlags, MountPropagationFlags, MountSetAttrFlags, OpenTreeFlags},
};
use std::{fs::File, io, marker::PhantomData, rc::Rc};

/// A detached mount-enforced read-only checkout. The original writable directory
/// descriptor is closed before publication. Its private backing remains solely
/// for cleanup; it is never passed to commands or exposed through this API.
#[derive(Debug)]
pub struct ReadOnlyResources {
    // Field order closes the read-only mount before backing cleanup.
    root: File,
    directory: tempfile::TempDir,
    _parent: File,
    _thread: PhantomData<Rc<()>>,
}

/// Failed detached setup retains the original staged owner. No attached mount
/// needs recovery and no read-only state is asserted. Cleanup or retry is explicit.
#[derive(Debug, thiserror::Error)]
#[error("context read-only mount setup: {source}")]
pub struct ContextMountFailure {
    source: io::Error,
    staged: StagedContext,
}
impl ContextMountFailure {
    pub fn source_error(&self) -> &io::Error {
        &self.source
    }
    pub fn into_parts(self) -> (io::Error, StagedContext) {
        (self.source, self.staged)
    }
    pub fn cleanup(self) -> Result<(), ContextError> {
        self.staged.close()
    }
}
impl StagedContext {
    pub fn read_only(self) -> Result<ReadOnlyContext, ContextMountFailure> {
        let result = (|| -> io::Result<File> {
            let tree = rustix::mount::open_tree(
                &self.resources.root,
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
                        | MountAttrFlags::MOUNT_ATTR_NODEV
                        | MountAttrFlags::MOUNT_ATTR_NOEXEC,
                    MountAttrFlags::empty(),
                    MountPropagationFlags::PRIVATE,
                ),
            )?;
            Ok(tree.into())
        })();
        let root = match result {
            Ok(root) => root,
            Err(source) => {
                return Err(ContextMountFailure {
                    source,
                    staged: self,
                });
            }
        };
        let StagedResources {
            directory,
            root: writable,
            _parent,
        } = self.resources;
        drop(writable);
        Ok(ReadOnlyContext {
            resources: ReadOnlyResources {
                root,
                directory,
                _parent,
                _thread: PhantomData,
            },
        })
    }
}
impl ReadOnlyContext {
    pub fn open(&self, path: &ContextPath) -> Result<File, ContextError> {
        let file = File::from(
            rustix::fs::openat2(
                &self.resources.root,
                path.as_ref(),
                OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS,
            )
            .map_err(io::Error::from)?,
        );
        if !file.metadata()?.is_file() {
            return Err(ContextError::Invalid(
                "selected checkout path is not a file",
            ));
        }
        Ok(file)
    }
    /// Release the detached view and remove backing names. Read-only file
    /// descriptors already returned may retain their inodes until last close;
    /// they do not make cleanup busy or regain writable mount authority.
    pub fn close(self) -> Result<(), ContextError> {
        let ReadOnlyResources {
            root,
            directory,
            _parent,
            ..
        } = self.resources;
        drop(root);
        let result = directory.close();
        drop(_parent);
        result.map_err(ContextError::from)
    }
}
