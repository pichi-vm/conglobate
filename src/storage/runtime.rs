// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Build-owned control mounts. No command receives these paths or descriptors.
use super::Workspace;
use crate::{
    anchor::{ControlMountNamespace, RecoveryDirectory},
    step::{SandboxDirectories, SetupBudget},
};
use rustix::{fs, mount};
use std::{
    fs::File,
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Control resources borrow the one RAM workspace. Explicit close is required
/// before publication. Abandonment retains mounts until dedicated VM disposal.
/// ```compile_fail
/// use conglobate::storage::Runtime;
/// fn cannot_close_runtime_with_sandbox(runtime: Runtime<'_>) {
///     let paths = runtime.paths().unwrap();
///     let sandbox = paths.sandbox();
///     runtime.close().unwrap();
///     drop(sandbox);
/// }
/// ```
#[derive(Debug)]
pub(crate) struct Runtime<'w> {
    recovery: RecoveryDirectory,
    mounts: RuntimeMounts<'w>,
}
#[derive(Debug)]
struct RuntimeMounts<'w> {
    scope: ControlMountNamespace,
    parent: File,
    name: std::ffi::OsString,
    workspace: &'w Workspace,
    backing: std::ffi::OsString,
    attached: Vec<PathBuf>,
}
#[derive(Debug, thiserror::Error)]
#[error("build runtime: {source}")]
pub(crate) struct RuntimeFailure<'w> {
    source: io::Error,
    retained: Option<RuntimeMounts<'w>>,
    deadline: Instant,
}
/// A scoped operation failed, or its runtime could not be closed. Cleanup
/// failures retain the runtime owner; successful operation output never escapes
/// alongside a failed close.
#[derive(Debug, thiserror::Error)]
pub(crate) enum RuntimeScopeFailure<'w, E> {
    #[error("{0}")]
    Create(RuntimeFailure<'w>),
    #[error("{0}")]
    Operation(E),
    #[error("{0}")]
    Cleanup(RuntimeFailure<'w>),
    #[error("{operation}; runtime cleanup: {cleanup}")]
    Both {
        operation: E,
        cleanup: RuntimeFailure<'w>,
    },
}
impl RuntimeFailure<'_> {
    /// Terminal pipeline boundary only: relinquish retry authority and preserve
    /// the I/O cause. Dropping retained owners does not unmount their resources;
    /// the dedicated VM must be disposed after this conversion.
    pub(crate) fn into_terminal_error(self) -> io::Error {
        let Self {
            source, retained, ..
        } = self;
        drop(retained);
        source
    }

    pub(crate) fn cleanup(self) -> Result<(), Self> {
        let Self {
            source,
            retained,
            deadline,
        } = self;
        if let Some(mut mounts) = retained
            && let Err(cleanup) = mounts.close(deadline)
        {
            return Err(Self {
                source: io::Error::other(format!("{source}; cleanup: {cleanup}")),
                retained: Some(mounts),
                deadline,
            });
        }
        Ok(())
    }
}
impl<'w> Runtime<'w> {
    /// Run trusted stage orchestration and require explicit runtime close on
    /// either returned outcome. Panic/crash still requires dedicated VM disposal.
    /// Returned output cannot borrow the runtime being closed.
    /// ```compile_fail
    /// use conglobate::storage::{Runtime, Workspace};
    /// fn cannot_escape(workspace: &Workspace) {
    ///     let paths = Runtime::scoped(workspace, std::path::Path::new("/run"),
    ///         |runtime| runtime.paths());
    /// }
    /// ```
    pub(crate) fn scoped<T, E>(
        workspace: &'w Workspace,
        parent: &Path,
        operation: impl FnOnce(&Self) -> Result<T, E>,
    ) -> Result<T, RuntimeScopeFailure<'w, E>> {
        let runtime = Self::create(workspace, parent).map_err(RuntimeScopeFailure::Create)?;
        let result = operation(&runtime);
        match (result, runtime.close()) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(error), Ok(())) => Err(RuntimeScopeFailure::Operation(error)),
            (Ok(_), Err(error)) => Err(RuntimeScopeFailure::Cleanup(error)),
            (Err(operation), Err(cleanup)) => Err(RuntimeScopeFailure::Both { operation, cleanup }),
        }
    }
    /// The deployment parent must be a real root-owned directory, not writable
    /// by group/other. Only an empty private attachment point is created there;
    /// mutable control data lives on the existing workspace tmpfs.
    pub(crate) fn create(workspace: &'w Workspace, parent: &Path) -> Result<Self, RuntimeFailure<'w>> {
        let budget = SetupBudget::begin();
        let before = |source| RuntimeFailure {
            source,
            retained: None,
            deadline: Instant::now() + Duration::from_secs(30),
        };
        let scope = ControlMountNamespace::capture().map_err(before)?;
        let parent = File::from(
            fs::open(
                parent,
                fs::OFlags::RDONLY
                    | fs::OFlags::DIRECTORY
                    | fs::OFlags::CLOEXEC
                    | fs::OFlags::NOFOLLOW,
                fs::Mode::empty(),
            )
            .map_err(|e| before(e.into()))?,
        );
        let metadata = parent.metadata().map_err(before)?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(before(io::Error::other(
                "runtime parent must be root-owned and protected",
            )));
        }
        budget.check().map_err(before)?;
        let target = tempfile::Builder::new()
            .prefix("conglobate-runtime-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))
            .map_err(before)?;
        let backing = tempfile::Builder::new()
            .prefix("runtime-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", workspace.root.as_raw_fd()))
            .map_err(before)?;
        // Both directories are exclusively acquired before retaining either.
        // A collision/failure must never authorize cleanup of an existing entry.
        let name = target
            .keep()
            .file_name()
            .expect("generated runtime name")
            .to_owned();
        let backing = backing
            .keep()
            .file_name()
            .expect("generated backing name")
            .to_owned();
        let mut mounts = RuntimeMounts {
            scope,
            parent,
            backing,
            name,
            workspace,
            attached: Vec::new(),
        };
        let acquired = (|| -> io::Result<RecoveryDirectory> {
            budget.check()?;
            let tree = mount::open_tree(
                &workspace.root,
                &mounts.backing,
                mount::OpenTreeFlags::OPEN_TREE_CLONE | mount::OpenTreeFlags::OPEN_TREE_CLOEXEC,
            )?;
            mounts.attach(tree.into(), mounts.path())?;
            for name in ["recovery", "volumes", "cgroups"] {
                std::fs::create_dir(mounts.path().join(name))?;
                std::fs::set_permissions(
                    mounts.path().join(name),
                    std::fs::Permissions::from_mode(0o700),
                )?;
            }
            {
                budget.check()?;
                let context = mount::fsopen("cgroup2", mount::FsOpenFlags::FSOPEN_CLOEXEC)?;
                mount::fsconfig_create(&context)?;
                let tree = mount::fsmount(
                    &context,
                    mount::FsMountFlags::FSMOUNT_CLOEXEC,
                    mount::MountAttrFlags::MOUNT_ATTR_NODEV
                        | mount::MountAttrFlags::MOUNT_ATTR_NOSUID
                        | mount::MountAttrFlags::MOUNT_ATTR_NOEXEC,
                )?;
                mounts.attach(tree.into(), mounts.path().join("cgroups"))?;
                budget.check()?;
            }
            std::fs::create_dir(mounts.path().join("cgroups").join(&mounts.name))?;
            std::fs::set_permissions(
                mounts.path().join("cgroups").join(&mounts.name),
                std::fs::Permissions::from_mode(0o700),
            )?;
            let recovery = RecoveryDirectory::open(&mounts.path().join("recovery"))?;
            budget.check()?;
            Ok(recovery)
        })();
        match acquired {
            Ok(recovery) => Ok(Self { recovery, mounts }),
            Err(source) => Err(RuntimeFailure {
                source,
                retained: Some(mounts),
                deadline: Instant::now() + Duration::from_secs(30),
            }),
        }
    }
    pub(crate) fn recovery(&self) -> &RecoveryDirectory {
        &self.recovery
    }
    pub(crate) fn paths(&self) -> io::Result<RuntimePaths<'_, 'w>> {
        let root = std::fs::canonicalize(self.mounts.path())?;
        Ok(RuntimePaths {
            runtime: self,
            volumes: root.join("volumes"),
            domains: root.join("cgroups").join(&self.mounts.name),
            recovery: root.join("recovery"),
        })
    }
    pub(crate) fn close(self) -> Result<(), RuntimeFailure<'w>> {
        let Self {
            recovery,
            mut mounts,
        } = self;
        drop(recovery);
        let deadline = Instant::now() + Duration::from_secs(30);
        mounts.close(deadline).map_err(|source| RuntimeFailure {
            source,
            retained: Some(mounts),
            deadline,
        })
    }
}
/// Borrowed by SandboxDirectories along with the runtime recovery lease.
#[derive(Debug)]
pub(crate) struct RuntimePaths<'r, 'w> {
    runtime: &'r Runtime<'w>,
    recovery: PathBuf,
    volumes: PathBuf,
    domains: PathBuf,
}
impl RuntimePaths<'_, '_> {
    pub(crate) fn sandbox(&self) -> SandboxDirectories<'_> {
        SandboxDirectories::new(self.runtime.recovery(), &self.volumes, &self.domains)
    }
    pub(crate) fn domains(&self) -> &Path {
        &self.domains
    }
    pub(crate) fn recovery(&self) -> &Path {
        &self.recovery
    }
    pub(crate) fn volumes(&self) -> &Path {
        &self.volumes
    }
}
impl RuntimeMounts<'_> {
    fn path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.parent.as_raw_fd())).join(&self.name)
    }
    fn attach(&mut self, tree: File, target: PathBuf) -> io::Result<()> {
        self.scope.verify_current()?;
        mount::move_mount(
            &tree,
            "",
            fs::CWD,
            &target,
            mount::MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
        )?;
        self.attached.push(target);
        Ok(())
    }
    fn remove_empty(path: &Path) -> io::Result<()> {
        match std::fs::remove_dir(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        }
    }
    fn check(deadline: Instant) -> io::Result<()> {
        if Instant::now() >= deadline {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "runtime cleanup deadline expired",
            ))
        } else {
            Ok(())
        }
    }
    fn close(&mut self, deadline: Instant) -> io::Result<()> {
        Self::check(deadline)?;
        self.scope.verify_current()?;
        // Never unmount pins or command mounts, even if the kernel would permit
        // detaching them. Empty-directory removal is the cleanup barrier.
        Self::remove_empty(&self.path().join("recovery"))?;
        Self::remove_empty(&self.path().join("volumes"))?;
        while let Some(target) = self.attached.last() {
            Self::check(deadline)?;
            if target.file_name().is_some_and(|n| n == "cgroups") {
                Self::remove_empty(&target.join(&self.name))?;
            }
            // Remove empty backing mountpoints before unmounting the workspace.
            if target == &self.path() {
                Self::remove_empty(&target.join("cgroups"))?;
            }
            mount::unmount(target, mount::UnmountFlags::NOFOLLOW)?;
            self.attached.pop();
        }
        fs::unlinkat(&self.workspace.root, &self.backing, fs::AtFlags::REMOVEDIR).or_else(|e| {
            if e == rustix::io::Errno::NOENT {
                Ok(())
            } else {
                Err(e)
            }
        })?;
        Self::remove_empty(&self.path())?;
        Self::check(deadline)
    }
}
impl Drop for RuntimeMounts<'_> {
    fn drop(&mut self) {
        if !self.attached.is_empty() {
            eprintln!(
                "runtime retained until VM disposal: {}",
                self.path().display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeFailure, RuntimeScopeFailure};
    use crate::pipeline::BuildError;
    use std::{error::Error, io, time::Instant};

    #[test]
    fn terminal_runtime_diagnostics_preserve_io_causes_and_phase() {
        for creation in [false, true] {
            let failure = RuntimeFailure {
                source: io::Error::from_raw_os_error(13),
                retained: None,
                deadline: Instant::now(),
            };
            let scope = if creation {
                RuntimeScopeFailure::Create(failure)
            } else {
                RuntimeScopeFailure::Cleanup(failure)
            };
            let terminal = BuildError::from(scope);
            let source = terminal
                .source()
                .unwrap()
                .downcast_ref::<io::Error>()
                .unwrap();
            assert_eq!(source.raw_os_error(), Some(13));
            assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
            assert_eq!(matches!(terminal, BuildError::RuntimeCreate(_)), creation);
            assert_eq!(matches!(terminal, BuildError::RuntimeCleanup(_)), !creation);
        }
    }

    #[test]
    fn terminal_runtime_failure_retains_both_typed_causes() {
        let operation = BuildError::Io(io::Error::from_raw_os_error(2));
        let failure = RuntimeScopeFailure::Both {
            operation,
            cleanup: RuntimeFailure {
                source: io::Error::from_raw_os_error(16),
                retained: None,
                deadline: Instant::now(),
            },
        };
        let terminal = BuildError::from(failure);
        let message = terminal.to_string();
        let BuildError::OperationAndCleanup { operation, cleanup } = terminal else {
            panic!("both failures must remain independently inspectable");
        };
        let BuildError::Io(operation) = *operation else {
            panic!("operation type was erased");
        };
        let BuildError::RuntimeCleanup(cleanup) = *cleanup else {
            panic!("cleanup phase was erased");
        };
        assert_eq!(operation.raw_os_error(), Some(2));
        assert_eq!(cleanup.raw_os_error(), Some(16));
        assert!(message.contains(&operation.to_string()));
        assert!(message.contains(&cleanup.to_string()));
    }

    #[test]
    fn successful_runtime_cleanup_preserves_operation_variant() {
        let terminal = BuildError::from(RuntimeScopeFailure::Operation(BuildError::Io(
            io::Error::from_raw_os_error(2),
        )));
        let BuildError::Io(source) = terminal else {
            panic!("successful cleanup must not wrap the operation error");
        };
        assert_eq!(source.raw_os_error(), Some(2));
    }
}
