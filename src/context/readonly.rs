// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ContextError, StagedContext};
use crate::path::ContextPath;
use rustix::{
    fs::{Mode, OFlags, ResolveFlags},
    mount::{MountFlags, UnmountFlags},
};
use std::{fs::File, io};

/// Owns a mount-enforced read-only checkout. It must be created in the builder's
/// private mount namespace, never a command namespace. No path is exposed.
#[derive(Debug)]
pub struct ReadOnlyContext {
    root: File,
    lease: MountLease,
}

/// Failed transitions retain a mounted checkout for explicit cleanup retry.
#[derive(Debug, thiserror::Error)]
pub enum ContextMountFailure {
    #[error("context setup failed before mount: {0}")]
    BeforeMount(ContextError),
    #[error("context mount operation failed: {source}")]
    Mounted {
        source: io::Error,
        recovery: MountRecovery,
    },
    #[error("context directory cleanup failed after unmount: {0}")]
    Cleanup(ContextError),
}
#[derive(Debug)]
pub struct MountRecovery {
    lease: MountLease,
}
impl ContextMountFailure {
    pub fn cleanup(self) -> Result<(), Self> {
        match self {
            Self::Mounted { recovery, .. } => recovery.lease.close(),
            error => Err(error),
        }
    }
}

#[derive(Debug)]
struct BackingDirectory {
    directory: tempfile::TempDir,
    // The proc-fd based directory pathname requires the pinned parent to live.
    _parent: File,
}
impl BackingDirectory {
    fn close(self) -> Result<(), ContextError> {
        let Self { directory, _parent } = self;
        let result = directory.close();
        drop(_parent);
        result.map_err(ContextError::from)
    }
    fn retain(self) {
        // Never recursively remove a directory while its mount is still live.
        let _ = self.directory.keep();
    }
}

#[derive(Debug)]
struct MountLease {
    // Option is solely a Drop ownership slot: successful close takes it once.
    backing: Option<BackingDirectory>,
    _thread: std::marker::PhantomData<std::rc::Rc<()>>,
}
impl MountLease {
    fn backing(&self) -> &BackingDirectory {
        self.backing.as_ref().expect("live mount lease")
    }
    fn close(mut self) -> Result<(), ContextMountFailure> {
        if let Err(error) =
            rustix::mount::unmount(self.backing().directory.path(), UnmountFlags::empty())
        {
            return Err(ContextMountFailure::Mounted {
                source: error.into(),
                recovery: MountRecovery { lease: self },
            });
        }
        self.backing
            .take()
            .expect("live mount lease")
            .close()
            .map_err(ContextMountFailure::Cleanup)
    }
}
impl Drop for MountLease {
    fn drop(&mut self) {
        if let Some(backing) = self.backing.take() {
            // Read-only fallback detaches the namespace entry; borrowed/open file
            // descriptors may keep the mount alive until their owners drop them.
            match rustix::mount::unmount(backing.directory.path(), UnmountFlags::DETACH) {
                Ok(()) => {
                    if let Err(error) = backing.close() {
                        eprintln!("context cleanup: {error}");
                    }
                }
                Err(error) => {
                    eprintln!("context mount retained after cleanup failure: {error}");
                    backing.retain();
                }
            }
        }
    }
}
impl StagedContext {
    pub fn read_only(self) -> Result<ReadOnlyContext, ContextMountFailure> {
        let Self {
            directory,
            root,
            _parent,
        } = self;
        // An old directory FD bypasses the new mount view, as the VM probe shows.
        drop(root);
        let backing = BackingDirectory { directory, _parent };
        if let Err(error) =
            rustix::mount::mount_bind(backing.directory.path(), backing.directory.path())
        {
            return Err(ContextMountFailure::BeforeMount(ContextError::Io(
                error.into(),
            )));
        }
        let lease = MountLease {
            backing: Some(backing),
            _thread: std::marker::PhantomData,
        };
        let result = (|| -> io::Result<File> {
            rustix::mount::mount_remount(
                lease.backing().directory.path(),
                MountFlags::BIND
                    | MountFlags::RDONLY
                    | MountFlags::NOSUID
                    | MountFlags::NODEV
                    | MountFlags::NOEXEC,
                "",
            )?;
            File::open(lease.backing().directory.path())
        })();
        match result {
            Ok(root) => Ok(ReadOnlyContext { root, lease }),
            Err(source) => Err(ContextMountFailure::Mounted {
                source,
                recovery: MountRecovery { lease },
            }),
        }
    }
}
impl ReadOnlyContext {
    pub fn open(&self, path: &ContextPath) -> Result<File, ContextError> {
        let file = File::from(
            rustix::fs::openat2(
                &self.root,
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
    pub fn close(self) -> Result<(), ContextMountFailure> {
        let Self { root, lease } = self;
        drop(root);
        lease.close()
    }
}
