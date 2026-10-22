// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Idmapped, IdmappedResources, Mount, MountState, private};
use rustix::mount::{MoveMountFlags, UnmountFlags};
use std::{
    fs::{self, File},
    io,
    marker::PhantomData,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::Path,
};

/// Private empty attachment point, anchored to a pinned tmpfs parent descriptor.
/// The trusted caller must keep other processes from mutating that parent.
#[derive(Debug)]
pub struct MountPoint {
    directory: tempfile::TempDir,
    _parent: File,
}
impl MountPoint {
    pub fn create(parent: &Path) -> io::Result<Self> {
        let parent = File::open(parent)?;
        if !parent.metadata()?.is_dir() || rustix::fs::fstatfs(&parent)?.f_type != 0x0102_1994 {
            return Err(io::Error::other("mount point requires a tmpfs directory"));
        }
        let directory = tempfile::Builder::new()
            .prefix("conglobate-mount-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))?;
        Ok(Self {
            directory,
            _parent: parent,
        })
    }
    fn close(self) -> io::Result<()> {
        let Self { directory, _parent } = self;
        let result = directory.close();
        drop(_parent);
        result
    }
}

#[derive(Debug)]
pub struct Attached;
impl private::Sealed for Attached {}
impl MountState for Attached {
    type Resources = AttachedResources;
}
#[derive(Debug)]
pub struct AttachedResources {
    // Drop this pin before the lease attempts a normal unmount.
    root: File,
    lease: AttachmentLease,
}
#[derive(Debug)]
struct AttachmentBacking {
    point: MountPoint,
    _namespace: File,
}
#[derive(Debug)]
struct AttachmentLease {
    // Only a destructor ownership slot, not a runtime state discriminant.
    backing: Option<AttachmentBacking>,
}
#[derive(Debug, thiserror::Error)]
#[error("attaching mapped mount: {source}")]
pub struct AttachmentFailure<'ns> {
    source: io::Error,
    mount: Mount<'ns, Idmapped>,
    point: MountPoint,
}
impl<'ns> AttachmentFailure<'ns> {
    pub fn into_parts(self) -> (io::Error, Mount<'ns, Idmapped>, MountPoint) {
        (self.source, self.mount, self.point)
    }
}
#[derive(Debug, thiserror::Error)]
pub enum UnmountFailure<'ns> {
    #[error("unmount failed: {source}")]
    Mounted {
        source: io::Error,
        recovery: UnmountRecovery<'ns>,
    },
    #[error("mount point cleanup failed after unmount: {0}")]
    Cleanup(io::Error),
}
#[derive(Debug)]
pub struct UnmountRecovery<'ns> {
    scope: &'ns crate::isolation::PrivateMountNamespace,
    lease: AttachmentLease,
    _thread: PhantomData<std::rc::Rc<()>>,
}
impl UnmountFailure<'_> {
    pub fn cleanup(self) -> Result<(), Self> {
        match self {
            Self::Mounted { recovery, .. } => recovery.lease.close(recovery.scope),
            error => Err(error),
        }
    }
}
impl<'ns> Mount<'ns, Idmapped> {
    /// Attach in the trusted builder's private mount namespace. No command may
    /// run until its separate containment and recovery protocol is established.
    pub fn attach(self, point: MountPoint) -> Result<Mount<'ns, Attached>, AttachmentFailure<'ns>> {
        if let Err(source) = rustix::mount::move_mount(
            &self.resources.root,
            "",
            rustix::fs::CWD,
            point.directory.path(),
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
        ) {
            return Err(AttachmentFailure {
                source: source.into(),
                mount: self,
                point,
            });
        }
        let IdmappedResources { root, _namespace } = self.resources;
        // All remaining work is infallible ownership transfer: no reopening gap.
        Ok(Mount {
            resources: AttachedResources {
                root,
                lease: AttachmentLease {
                    backing: Some(AttachmentBacking { point, _namespace }),
                },
            },
            _scope: self._scope,
            _thread: PhantomData,
        })
    }
}
impl<'ns> Mount<'ns, Attached> {
    pub fn root_metadata(&self) -> io::Result<fs::Metadata> {
        self.resources.root.metadata()
    }
    /// Cleanly remove this view. This is not filesystem sealing or proof that
    /// another mount/command cannot still write the underlying filesystem.
    pub fn close(self) -> Result<(), UnmountFailure<'ns>> {
        let AttachedResources { root, lease } = self.resources;
        drop(root);
        lease.close(self._scope)
    }
}
impl AttachmentLease {
    fn close<'ns>(
        mut self,
        scope: &'ns crate::isolation::PrivateMountNamespace,
    ) -> Result<(), UnmountFailure<'ns>> {
        let backing = self.backing.as_ref().expect("live attachment lease");
        if let Err(source) =
            rustix::mount::unmount(backing.point.directory.path(), UnmountFlags::empty())
        {
            return Err(UnmountFailure::Mounted {
                source: source.into(),
                recovery: UnmountRecovery {
                    lease: self,
                    scope,
                    _thread: PhantomData,
                },
            });
        }
        let backing = self.backing.take().expect("live attachment lease");
        backing.point.close().map_err(UnmountFailure::Cleanup)
    }
}
impl Drop for AttachmentLease {
    fn drop(&mut self) {
        if let Some(backing) = self.backing.take() {
            match rustix::mount::unmount(backing.point.directory.path(), UnmountFlags::DETACH) {
                Ok(()) => {
                    if let Err(error) = backing.point.close() {
                        eprintln!("mount point cleanup: {error}");
                    }
                }
                Err(error) => {
                    // Never recurse through a still-attached writable filesystem.
                    // Preserve the namespace pin too. Durable crash anchoring is
                    // a separate prerequisite before command admission.
                    eprintln!("attachment retained after cleanup failure: {error}");
                    std::mem::forget(backing);
                }
            }
        }
    }
}
