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
    output_namespace: Option<File>,
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
        let IdmappedResources {
            root,
            _namespace,
            output_namespace,
        } = self.resources;
        // All remaining work is infallible ownership transfer: no reopening gap.
        Ok(Mount {
            resources: AttachedResources {
                root,
                lease: AttachmentLease {
                    backing: Some(AttachmentBacking {
                        point,
                        _namespace,
                        output_namespace,
                    }),
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
        let backing = self.backing.as_mut().expect("live attachment lease");
        if backing.output_namespace.is_some() {
            if let Err(source) = rustix::mount::unmount(
                backing.point.directory.path().join("mnt"),
                UnmountFlags::empty(),
            ) {
                return Err(UnmountFailure::Mounted {
                    source: source.into(),
                    recovery: UnmountRecovery {
                        lease: self,
                        scope,
                        _thread: PhantomData,
                    },
                });
            }
            backing.output_namespace.take();
        }
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

/// A mapped view retained by an executing trusted setup child. It cannot be
/// explicitly unmounted until that child is reaped. Descendant containment and
/// policy lifetime remain the enclosing step's responsibility.
///
/// ```compile_fail
/// use conglobate::filesystem::{Mount, InUse};
/// fn cannot_unmount_running_setup(mount: Mount<'_, InUse>) {
///     mount.close();
/// }
/// ```
#[derive(Debug)]
pub struct InUse;
impl private::Sealed for InUse {}
impl MountState for InUse {
    type Resources = InUseResources;
}
#[derive(Debug)]
pub struct InUseResources {
    process: crate::process::Process<crate::process::Running>,
    attached: AttachedResources,
}
#[derive(Debug, thiserror::Error)]
#[error("mounted-root setup launch: {source}")]
pub struct RootLaunchFailure<'ns> {
    source: io::Error,
    mount: Mount<'ns, Attached>,
}
impl<'ns> RootLaunchFailure<'ns> {
    pub fn into_parts(self) -> (io::Error, Mount<'ns, Attached>) {
        (self.source, self.mount)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("mounted-root setup wait: {source}")]
pub struct RootWaitFailure<'ns> {
    source: crate::process::WaitError,
    mount: Mount<'ns, InUse>,
}
impl<'ns> RootWaitFailure<'ns> {
    pub fn into_parts(self) -> (crate::process::WaitError, Mount<'ns, InUse>) {
        (self.source, self.mount)
    }
}
impl<'ns> Mount<'ns, Attached> {
    /// Launch trusted setup with its cwd set to this owned image view. The
    /// kernel carries cwd through subsequent namespace cloning; root entry must
    /// capture it afterward. No generated pathname or raw descriptor escapes.
    /// Image commands must not run until the child completes its sandbox setup.
    pub fn launch(
        self,
        mut setup: std::process::Command,
    ) -> Result<Mount<'ns, InUse>, RootLaunchFailure<'ns>> {
        let point = &self
            .resources
            .lease
            .backing
            .as_ref()
            .expect("attached mount")
            .point;
        setup.current_dir(point.directory.path());
        let child = match setup.spawn() {
            Ok(child) => child,
            Err(source) => {
                return Err(RootLaunchFailure {
                    source,
                    mount: self,
                });
            }
        };
        Ok(Mount {
            resources: InUseResources {
                process: child.into(),
                attached: self.resources,
            },
            _scope: self._scope,
            _thread: self._thread,
        })
    }
}
impl<'ns> Mount<'ns, InUse> {
    pub(crate) fn relay_diagnostics(&mut self) -> io::Result<crate::process::DiagnosticRelay> {
        self.resources.process.relay_diagnostics()
    }

    /// Trusted contained helper only; the external supervisor owns the command
    /// deadline and cancels this helper and all descendants together.
    pub(crate) fn wait_under_supervisor(
        self,
    ) -> Result<(Mount<'ns, Attached>, std::process::ExitStatus), RootWaitFailure<'ns>> {
        self.finish(|process| process.wait_under_supervisor())
    }

    /// Collect the direct setup child's status. This is not proof of descendant
    /// quiescence or successful filesystem finalization. Failure retains both
    /// mount and process ownership; callers can retry or request cancellation.
    pub fn wait_until(
        self,
        deadline: std::time::Instant,
    ) -> Result<(Mount<'ns, Attached>, std::process::ExitStatus), RootWaitFailure<'ns>> {
        self.finish(|process| process.wait_until(deadline))
    }
    /// Terminate and reap this direct setup child. This does not cancel the
    /// entire command cgroup or prove descendant quiescence.
    pub fn terminate_until(
        self,
        deadline: std::time::Instant,
    ) -> Result<(Mount<'ns, Attached>, std::process::ExitStatus), RootWaitFailure<'ns>> {
        self.finish(|process| process.terminate_until(deadline))
    }
    fn finish(
        self,
        wait: impl FnOnce(
            crate::process::Process<crate::process::Running>,
        ) -> Result<
            crate::process::Process<crate::process::Reaped>,
            crate::process::WaitFailure,
        >,
    ) -> Result<(Mount<'ns, Attached>, std::process::ExitStatus), RootWaitFailure<'ns>> {
        let InUseResources { process, attached } = self.resources;
        match wait(process) {
            Ok(process) => Ok((
                Mount {
                    resources: attached,
                    _scope: self._scope,
                    _thread: self._thread,
                },
                process.status(),
            )),
            Err(failure) => {
                let (source, process) = failure.into_parts();
                Err(RootWaitFailure {
                    source,
                    mount: Mount {
                        resources: InUseResources { process, attached },
                        _scope: self._scope,
                        _thread: self._thread,
                    },
                })
            }
        }
    }
}
