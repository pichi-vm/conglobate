// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Idmapped, IdmappedResources, Mount, MountState, private};
use rustix::mount::{MoveMountFlags, UnmountFlags};
use std::{
    fs::File,
    io,
    marker::PhantomData,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::{Path, PathBuf},
};

/// Existing supervisor-owned attachment directory. The helper overlays it only
/// in its private mount namespace; it creates no shared directory that could
/// outlive cancellation. The supervisor retains the underlying directory and
/// must prevent mutation of its parent until this helper is quiescent.
#[derive(Debug)]
pub(crate) struct MountPoint {
    parent: File,
    name: std::ffi::OsString,
}
impl MountPoint {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let name = path.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "attachment requires a directory name",
            )
        })?;
        let parent = File::from(rustix::fs::open(
            path.parent()
                .ok_or_else(|| io::Error::other("attachment parent missing"))?,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?);
        let target = File::from(rustix::fs::openat(
            &parent,
            name,
            rustix::fs::OFlags::PATH
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?);
        for directory in [&parent, &target] {
            let metadata = directory.metadata()?;
            if metadata.uid() != 0
                || metadata.mode() & 0o022 != 0
                || rustix::fs::fstatfs(directory)?.f_type != 0x0102_1994
            {
                return Err(io::Error::other(
                    "attachment requires protected root-owned tmpfs directories",
                ));
            }
        }
        Ok(Self {
            parent,
            name: name.to_owned(),
        })
    }
    fn path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.parent.as_raw_fd())).join(&self.name)
    }
}

#[derive(Debug)]
pub(crate) struct Attached;
impl private::Sealed for Attached {}
impl MountState for Attached {
    type Resources = AttachedResources;
}
#[derive(Debug)]
pub(crate) struct AttachedResources {
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
pub(crate) enum AttachmentFailure<'ns> {
    #[error("attaching mapped root: {source}")]
    Root {
        source: io::Error,
        mount: Mount<'ns, Idmapped>,
        point: MountPoint,
    },
    #[error("attaching bootstrap output: {source}")]
    Output {
        source: io::Error,
        mount: Mount<'ns, Attached>,
        output: Mount<'ns, Idmapped>,
    },
}
impl<'ns> Mount<'ns, Idmapped> {
    /// Attach in the trusted builder's private mount namespace. No command may
    /// run until its separate containment and recovery protocol is established.
    pub(crate) fn attach(
        self,
        point: MountPoint,
    ) -> Result<Mount<'ns, Attached>, AttachmentFailure<'ns>> {
        if let Err(source) = self._scope.verify_current() {
            return Err(AttachmentFailure::Root {
                source,
                mount: self,
                point,
            });
        }
        if let Err(source) = rustix::mount::move_mount(
            &self.resources.root,
            "",
            rustix::fs::CWD,
            point.path(),
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
        ) {
            return Err(AttachmentFailure::Root {
                source: source.into(),
                mount: self,
                point,
            });
        }
        let IdmappedResources {
            root,
            _namespace,
            output,
        } = self.resources;
        let mounted = Mount {
            resources: AttachedResources {
                root,
                lease: AttachmentLease {
                    backing: Some(AttachmentBacking {
                        point,
                        _namespace,
                        output_namespace: None,
                    }),
                },
            },
            _scope: self._scope,
            _thread: PhantomData,
        };
        match output {
            Some(output) => mounted.attach_output(Mount {
                resources: IdmappedResources {
                    root: output.root,
                    _namespace: output.namespace,
                    output: None,
                },
                _scope: self._scope,
                _thread: PhantomData,
            }),
            None => Ok(mounted),
        }
    }
}
impl<'ns> Mount<'ns, Attached> {
    fn attach_output(
        mut self,
        output: Mount<'ns, Idmapped>,
    ) -> Result<Self, AttachmentFailure<'ns>> {
        let result = (|| -> io::Result<()> {
            let target = rustix::fs::openat2(
                &self.resources.root,
                "mnt",
                rustix::fs::OFlags::PATH
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
                rustix::fs::ResolveFlags::BENEATH
                    | rustix::fs::ResolveFlags::NO_SYMLINKS
                    | rustix::fs::ResolveFlags::NO_XDEV,
            )?;
            rustix::mount::move_mount(
                &output.resources.root,
                "",
                &target,
                "",
                MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
            )?;
            Ok(())
        })();
        if let Err(source) = result {
            return Err(AttachmentFailure::Output {
                source,
                mount: self,
                output,
            });
        }
        let IdmappedResources {
            root,
            _namespace,
            output: _,
        } = output.resources;
        drop(root);
        self.resources
            .lease
            .backing
            .as_mut()
            .expect("attached root")
            .output_namespace = Some(_namespace);
        Ok(self)
    }
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn root_metadata(&self) -> io::Result<std::fs::Metadata> {
        self.resources.root.metadata()
    }
    /// Cleanly remove this view. This is not filesystem sealing or proof that
    /// another mount/command cannot still write the underlying filesystem.
    pub(crate) fn close(self) -> io::Result<()> {
        let AttachedResources { root, lease } = self.resources;
        drop(root);
        lease.close(self._scope)
    }
}
impl AttachmentLease {
    fn close(mut self, scope: &crate::isolation::PrivateMountNamespace) -> io::Result<()> {
        let result = (|| -> io::Result<()> {
            scope.verify_current()?;
            let backing = self.backing.as_mut().expect("live attachment lease");
            if backing.output_namespace.is_some() {
                rustix::mount::unmount(backing.point.path().join("mnt"), UnmountFlags::empty())?;
                backing.output_namespace.take();
            }
            rustix::mount::unmount(backing.point.path(), UnmountFlags::empty())?;
            Ok(())
        })();
        if let Err(error) = result {
            std::mem::forget(self);
            return Err(error);
        }
        self.backing.take();
        Ok(())
    }
}
impl Drop for AttachmentLease {
    fn drop(&mut self) {
        if let Some(backing) = self.backing.take() {
            match rustix::mount::unmount(backing.point.path(), UnmountFlags::DETACH) {
                Ok(()) => drop(backing),
                Err(error) => {
                    // Retain attachment and namespace descriptors on failure.
                    // The supervisor must prove quiescence or dispose of the VM.
                    eprintln!("attachment retained after cleanup failure: {error}");
                    std::mem::forget(backing);
                }
            }
        }
    }
}

/// A mapped view retained by an executing trusted setup child. It cannot be
/// explicitly unmounted until that child is reaped. Descendant containment and
/// storage lifetime remain the enclosing step's responsibility.
///
#[derive(Debug)]
pub(crate) struct InUse;
impl private::Sealed for InUse {}
impl MountState for InUse {
    type Resources = InUseResources;
}
#[derive(Debug)]
pub(crate) struct InUseResources {
    process: crate::process::Process<crate::process::Running>,
    attached: AttachedResources,
}
#[derive(Debug, thiserror::Error)]
#[error("mounted-root setup wait: {source}")]
pub(crate) struct RootWaitFailure<'ns> {
    source: crate::process::WaitError,
    mount: Mount<'ns, InUse>,
}
impl<'ns> RootWaitFailure<'ns> {
    pub(crate) fn into_parts(self) -> (crate::process::WaitError, Mount<'ns, InUse>) {
        (self.source, self.mount)
    }
}
impl<'ns> Mount<'ns, Attached> {
    /// Launch trusted setup with its cwd set to this owned image view. The
    /// kernel carries cwd through subsequent namespace cloning; root entry must
    /// capture it afterward. No generated pathname or raw descriptor escapes.
    /// Image commands must not run until the child completes its sandbox setup.
    pub(crate) fn launch(self, mut setup: std::process::Command) -> io::Result<Mount<'ns, InUse>> {
        let point = &self
            .resources
            .lease
            .backing
            .as_ref()
            .expect("attached mount")
            .point;
        setup.current_dir(point.path());
        let child = setup.spawn()?;
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
    pub(crate) fn wait_until(
        self,
        deadline: std::time::Instant,
    ) -> Result<(Mount<'ns, Attached>, std::process::ExitStatus), RootWaitFailure<'ns>> {
        self.finish(|process| process.wait_until(deadline))
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
