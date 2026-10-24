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
impl<'w> Runtime<'w> {
    /// Run trusted stage orchestration and require explicit runtime close on
    /// either returned outcome. Panic/crash still requires dedicated VM disposal.
    /// Returned output cannot borrow the runtime being closed.
    pub(crate) fn scoped<T>(
        workspace: &'w Workspace,
        parent: &Path,
        operation: impl FnOnce(&Self) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let runtime = Self::create(workspace, parent)?;
        let result = operation(&runtime);
        match (result, runtime.close()) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error.into()),
            (Err(error), Err(cleanup)) => Err(error.context(format!("runtime cleanup: {cleanup}"))),
        }
    }
    /// The deployment parent must be a real root-owned directory, not writable
    /// by group/other. Only an empty private attachment point is created there;
    /// mutable control data lives on the existing workspace tmpfs.
    pub(crate) fn create(workspace: &'w Workspace, parent: &Path) -> anyhow::Result<Self> {
        let budget = SetupBudget::begin();
        let scope = ControlMountNamespace::capture()?;
        let parent = File::from(fs::open(
            parent,
            fs::OFlags::RDONLY | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC | fs::OFlags::NOFOLLOW,
            fs::Mode::empty(),
        )?);
        let metadata = parent.metadata()?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(io::Error::other("runtime parent must be root-owned and protected").into());
        }
        budget.check()?;
        let target = tempfile::Builder::new()
            .prefix("conglobate-runtime-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))?;
        let backing = tempfile::Builder::new()
            .prefix("runtime-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", workspace.root.as_raw_fd()))?;
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
            Err(source) => {
                let cleanup = mounts.close(Instant::now() + Duration::from_secs(30));
                match cleanup {
                    Ok(()) => Err(source.into()),
                    Err(cleanup) => {
                        std::mem::forget(mounts);
                        Err(anyhow::Error::new(source)
                            .context(format!("runtime setup cleanup: {cleanup}")))
                    }
                }
            }
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
            #[cfg(feature = "test-fixtures")]
            recovery: root.join("recovery"),
        })
    }
    pub(crate) fn close(self) -> io::Result<()> {
        let Self {
            recovery,
            mut mounts,
        } = self;
        drop(recovery);
        if let Err(error) = mounts.close(Instant::now() + Duration::from_secs(30)) {
            std::mem::forget(mounts);
            return Err(error);
        }
        Ok(())
    }
}
/// Borrowed by SandboxDirectories along with the runtime recovery lease.
#[derive(Debug)]
pub(crate) struct RuntimePaths<'r, 'w> {
    runtime: &'r Runtime<'w>,
    #[cfg(feature = "test-fixtures")]
    recovery: PathBuf,
    volumes: PathBuf,
    domains: PathBuf,
}
impl RuntimePaths<'_, '_> {
    pub(crate) fn sandbox(&self) -> SandboxDirectories<'_> {
        SandboxDirectories::new(self.runtime.recovery(), &self.volumes, &self.domains)
    }
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn domains(&self) -> &Path {
        &self.domains
    }
    #[cfg(feature = "test-fixtures")]
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
