// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{PartitionedSnapshot, SnapshotMapping};
use crate::anchor::RecoveryDirectory;
use rustix::{fs, mount};
use std::{
    fs::File,
    io,
    marker::PhantomData,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::{Path, PathBuf},
    rc::Rc,
};
mod stage;
use stage::{StageAttachments, StageMounts};

mod private {
    pub(crate) trait Sealed {}
}
pub(crate) trait VolumeState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub(crate) struct Unmounted;
#[derive(Debug)]
pub(crate) struct Mounted;
impl private::Sealed for Unmounted {}
impl private::Sealed for Mounted {}
impl VolumeState for Unmounted {
    type Resources = ();
}
impl VolumeState for Mounted {
    type Resources = MountedResources;
}
#[derive(Debug)]
pub(crate) struct MountedResources {
    children: StageAttachments,
    root: File,
    lease: MountLease,
}

/// Filesystem mount ownership coupled to its writable snapshot. Unmounted is
/// not a sealing receipt or proof that no independent kernel consumer exists.
/// Root views borrow this owner; no raw root or backing descriptors escape.
/// Abandoning a mounted volume attempts a lazy detach as a destructor fallback;
/// it neither proves flush completion nor returns an Unmounted value. Use the
/// explicit fallible unmount and finalization path before sealing output.
#[derive(Debug)]
pub(crate) struct Volume<'ns, 'origin, 'workspace, S: VolumeState> {
    // Mount resources must drop before the underlying snapshot.
    resources: S::Resources,
    stage: StageMounts,
    snapshot: PartitionedSnapshot<'origin, 'workspace>,
    scope: &'ns RecoveryDirectory,
    _thread: PhantomData<Rc<()>>,
}
/// Detached tree cloned in the volume's namespace, including owned stage mounts. Assignment to
/// a private command namespace preserves the borrow of its mounted volume.
#[derive(Debug)]
pub(crate) struct VolumeView<'volume> {
    root: File,
    _volume: PhantomData<&'volume ()>,
    _thread: PhantomData<Rc<()>>,
}
impl<'volume> VolumeView<'volume> {
    pub(crate) fn descriptor(&self) -> io::Result<File> {
        self.root.try_clone()
    }
}
impl<'ns, 'a, 'w> Volume<'ns, 'a, 'w, Unmounted> {
    pub(crate) fn into_snapshot(self) -> io::Result<SnapshotMapping<'a, 'w>> {
        if let Err(error) = self.stage.close(self.scope) {
            std::mem::forget(self.snapshot);
            return Err(error);
        }
        self.snapshot.into_snapshot()
    }
    pub(crate) fn with_mounts(
        mut self,
        mounts: &std::collections::BTreeMap<
            crate::contract::recipe::ImagePath,
            crate::contract::recipe::Mount,
        >,
        parent: &Path,
        creation: bool,
    ) -> io::Result<Self> {
        match self
            .stage
            .populate(mounts, &self.snapshot, self.scope, parent, creation)
        {
            Ok(()) => Ok(self),
            Err(error) => Err(self.discard(error)),
        }
    }
    fn discard(self, error: io::Error) -> io::Error {
        match self
            .into_snapshot()
            .and_then(|snapshot| snapshot.close())
            .and_then(|backing| backing.detach())
        {
            Ok(_) => error,
            Err(cleanup) => io::Error::other(format!("{error}; stage cleanup: {cleanup}")),
        }
    }
    /// Observe snapshot health after all filesystem writes have been flushed.
    pub(crate) fn validate_snapshot(&self) -> std::io::Result<()> {
        self.snapshot.validate()
    }
    /// Mount under supervisor ownership in the verified init namespace.
    /// Command execution still requires its own private namespace and view.
    pub(crate) fn in_init_namespace(
        snapshot: SnapshotMapping<'a, 'w>,
        scope: &'ns RecoveryDirectory,
    ) -> io::Result<Self> {
        scope.verify_current()?;
        let volume = Self {
            resources: (),
            stage: StageMounts::default(),
            snapshot: PartitionedSnapshot::open(snapshot)?,
            scope,
            _thread: PhantomData,
        };
        volume.scope.verify_current()?;
        Ok(volume)
    }
    /// Initial backend is ext4, with fixed nodev/nosuid mount policy. The parent
    /// must be trusted tmpfs in this volume's mount namespace; it stores only the
    /// empty attachment directory, not image data. No plan mount options pass through.
    pub(crate) fn mount_ext4(self, parent: &Path) -> io::Result<Volume<'ns, 'a, 'w, Mounted>> {
        let attempt = (|| -> io::Result<MountedResources> {
            self.scope.verify_current()?;
            let point = Point::create(parent)?;
            let source = self.snapshot.open_root()?;
            let context = mount::fsopen("ext4", mount::FsOpenFlags::FSOPEN_CLOEXEC)?;
            mount::fsconfig_set_string(
                &context,
                "source",
                format!("/proc/self/fd/{}", source.as_raw_fd()),
            )?;
            mount::fsconfig_create(&context)?;
            let tree = mount::fsmount(
                &context,
                mount::FsMountFlags::FSMOUNT_CLOEXEC,
                mount::MountAttrFlags::MOUNT_ATTR_NOSUID | mount::MountAttrFlags::MOUNT_ATTR_NODEV,
            )?;
            let root = File::from(fs::openat(
                &tree,
                ".",
                fs::OFlags::RDONLY | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC,
                fs::Mode::empty(),
            )?);
            mount::move_mount(
                &tree,
                "",
                &point.parent,
                &point.name,
                mount::MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
            )?;
            // Infallible ownership transfer after attachment. Point's destructor
            // only removes an empty directory; it never traverses mounted data.
            Ok(MountedResources {
                children: StageAttachments::default(),
                root,
                lease: MountLease { point: Some(point) },
            })
        })();
        let resources = match attempt {
            Ok(resources) => resources,
            Err(error) => return Err(self.discard(error)),
        };
        let mut volume: Volume<'ns, 'a, 'w, Mounted> = Volume {
            resources,
            stage: self.stage,
            snapshot: self.snapshot,
            scope: self.scope,
            _thread: PhantomData,
        };
        match volume.stage.attach(
            &volume.resources.root,
            volume.scope,
            &mut volume.resources.children,
        ) {
            Ok(()) => Ok(volume),
            Err(error) => Err(match volume.unmount() {
                Ok(unmounted) => unmounted.discard(error),
                Err(cleanup) => io::Error::other(format!("{error}; root cleanup: {cleanup}")),
            }),
        }
    }
}
impl<'ns, 'a, 'w> Volume<'ns, 'a, 'w, Mounted> {
    /// Copy while exclusively borrowing this volume. Outstanding root views
    /// prevent this borrow, so no command may concurrently use the filesystem.
    pub(crate) fn copy(
        &mut self,
        context: &crate::context::CapturedContext,
        directive: &crate::plan::Copy,
    ) -> Result<(), crate::copy::CopyError> {
        self.scope.verify_current()?;
        crate::copy::CopyDestination::new(&self.resources.root).copy(context, directive)
    }

    pub(crate) fn copy_image(
        &mut self,
        output: Option<&mut Volume<'_, '_, '_, Mounted>>,
        directive: &crate::plan::Copy,
        source: &Volume<'_, '_, '_, Mounted>,
        budget: &crate::step::SetupBudget,
    ) -> Result<(), crate::copy::CopyError> {
        self.scope.verify_current()?;
        source.scope.verify_current()?;
        match output {
            Some(output) => {
                output.scope.verify_current()?;
                self.prepare_output_mountpoint()?;
                crate::copy::CopyDestination::bootstrap(
                    &self.resources.root,
                    &output.resources.root,
                )
                .image(&source.resources.root, directive, budget)
            }
            None => crate::copy::CopyDestination::new(&self.resources.root).image(
                &source.resources.root,
                directive,
                budget,
            ),
        }
    }
    pub(crate) fn extract_artifact<'out>(
        &mut self,
        path: &str,
        workspace: &'out super::Workspace,
        budget: &crate::step::SetupBudget,
    ) -> std::io::Result<crate::firmware::FirmwareFile<'out>> {
        self.scope.verify_current()?;
        crate::firmware::FirmwareFile::capture(
            crate::firmware::FirmwareFile::open(&self.resources.root, path)?,
            workspace,
            budget,
        )
    }
    pub(crate) fn copy_with_output(
        &mut self,
        output: &mut Volume<'_, '_, '_, Mounted>,
        context: &crate::context::CapturedContext,
        directive: &crate::plan::Copy,
    ) -> Result<(), crate::copy::CopyError> {
        self.scope.verify_current()?;
        output.scope.verify_current()?;
        self.prepare_output_mountpoint()?;
        crate::copy::CopyDestination::bootstrap(&self.resources.root, &output.resources.root)
            .copy(context, directive)
    }

    pub(crate) fn validate_ownership(
        &self,
        uid: crate::isolation::IdRange,
        gid: crate::isolation::IdRange,
        deadline: std::time::Instant,
    ) -> io::Result<()> {
        self.scope.verify_current()?;
        // A nonrecursive view exposes the persisted root without stage mounts.
        // Validate every owned temporary filesystem independently below.
        let tree = mount::open_tree(
            &self.resources.root,
            "",
            mount::OpenTreeFlags::AT_EMPTY_PATH
                | mount::OpenTreeFlags::OPEN_TREE_CLONE
                | mount::OpenTreeFlags::OPEN_TREE_CLOEXEC,
        )?;
        let root = File::from(fs::openat(
            &tree,
            ".",
            fs::OFlags::RDONLY | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC,
            fs::Mode::empty(),
        )?);
        super::ownership::ImageOwnership::new(uid, gid, deadline).validate(&root)?;
        self.stage.validate_ownership(uid, gid, deadline)
    }
    pub(crate) fn validate_command_root(&self) -> io::Result<()> {
        self.scope.verify_current()?;
        crate::isolation::RuntimeTargets::resolve(&self.resources.root).map(|_| ())
    }

    pub(crate) fn view(&self) -> io::Result<VolumeView<'_>> {
        self.scope.verify_current()?;
        let root = mount::open_tree(
            &self.resources.root,
            "",
            mount::OpenTreeFlags::AT_EMPTY_PATH
                | mount::OpenTreeFlags::AT_RECURSIVE
                | mount::OpenTreeFlags::OPEN_TREE_CLONE
                | mount::OpenTreeFlags::OPEN_TREE_CLOEXEC,
        )?;
        Ok(VolumeView {
            root: root.into(),
            _volume: PhantomData,
            _thread: PhantomData,
        })
    }
    pub(crate) fn unmount(self) -> io::Result<Volume<'ns, 'a, 'w, Unmounted>> {
        let MountedResources {
            root,
            lease,
            children,
        } = self.resources;
        if let Err(error) = children.close(self.scope) {
            std::mem::forget((root, lease, self.stage, self.snapshot));
            return Err(error);
        }
        if let Err(error) = lease.close(root, self.scope) {
            std::mem::forget((self.stage, self.snapshot));
            return Err(error);
        }
        Ok(Volume {
            resources: (),
            stage: self.stage,
            snapshot: self.snapshot,
            scope: self.scope,
            _thread: PhantomData,
        })
    }
}

#[derive(Debug)]
struct Point {
    parent: File,
    name: std::ffi::OsString,
    remove: bool,
}
impl Point {
    fn create(parent: &Path) -> io::Result<Self> {
        let parent = File::from(fs::open(
            parent,
            fs::OFlags::RDONLY | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC | fs::OFlags::NOFOLLOW,
            fs::Mode::empty(),
        )?);
        if fs::fstatfs(&parent)?.f_type != 0x0102_1994 {
            return Err(io::Error::other(
                "volume attachment requires a tmpfs parent",
            ));
        }
        let directory = tempfile::Builder::new()
            .prefix("conglobate-volume-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))?;
        let path = directory.keep();
        Ok(Self {
            name: path
                .file_name()
                .expect("generated directory name")
                .to_owned(),
            parent,
            remove: true,
        })
    }
    fn image(root: &File, path: &str) -> io::Result<Self> {
        let mut parent = root.try_clone()?;
        let components = path.trim_start_matches('/').split('/').collect::<Vec<_>>();
        for (index, name) in components.iter().enumerate() {
            if name.is_empty() || *name == "." || *name == ".." {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "invalid stage mount component",
                ));
            }
            match fs::mkdirat(&parent, *name, fs::Mode::from_raw_mode(0o755)) {
                Ok(()) => fs::chmodat(
                    &parent,
                    *name,
                    fs::Mode::from_raw_mode(0o755),
                    fs::AtFlags::empty(),
                )?,
                Err(rustix::io::Errno::EXIST) => (),
                Err(error) => return Err(error.into()),
            }
            let directory = File::from(fs::openat2(
                &parent,
                *name,
                fs::OFlags::PATH | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC,
                fs::Mode::empty(),
                fs::ResolveFlags::BENEATH | fs::ResolveFlags::NO_SYMLINKS,
            )?);
            if index + 1 == components.len() {
                return Ok(Self {
                    parent,
                    name: (*name).into(),
                    remove: false,
                });
            }
            parent = directory;
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "stage mount cannot replace root",
        ))
    }
    fn path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.parent.as_raw_fd())).join(&self.name)
    }
    fn remove(&self) -> io::Result<()> {
        if !self.remove {
            return Ok(());
        }
        match fs::unlinkat(&self.parent, &self.name, fs::AtFlags::REMOVEDIR) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}
impl Drop for Point {
    fn drop(&mut self) {
        if let Err(error) = self.remove() {
            eprintln!("volume attachment directory retained: {error}");
        }
    }
}
#[derive(Debug)]
struct MountLease {
    point: Option<Point>,
}
impl MountLease {
    fn close(mut self, root: File, scope: &RecoveryDirectory) -> io::Result<()> {
        if let Err(error) = scope
            .verify_current()
            .and_then(|()| fs::syncfs(&root).map_err(Into::into))
        {
            std::mem::forget((root, self));
            return Err(error);
        }
        drop(root);
        let point = self.point.as_ref().expect("attached mount");
        let result = scope
            .verify_current()
            .and_then(|()| {
                mount::unmount(point.path(), mount::UnmountFlags::NOFOLLOW).map_err(Into::into)
            })
            .and_then(|()| point.remove());
        if let Err(error) = result {
            std::mem::forget(self);
            return Err(error);
        }
        self.point.take();
        Ok(())
    }
}
impl Drop for MountLease {
    fn drop(&mut self) {
        if let Some(point) = self.point.take() {
            // Fallback only: never return an Unmounted value after lazy detach.
            if let Err(error) = mount::unmount(
                point.path(),
                mount::UnmountFlags::DETACH | mount::UnmountFlags::NOFOLLOW,
            ) {
                eprintln!("volume mount retained until VM disposal: {error}");
                std::mem::forget(point);
            }
        }
    }
}

impl Volume<'_, '_, '_, Mounted> {
    /// Reserved bootstrap mountpoint, created through the original unmapped
    /// filesystem so image UID/GID 0 remain UID/GID 0 on disk. Called before
    /// any command admission; reject symlinks and preexisting submounts.
    pub(crate) fn prepare_output_mountpoint(&self) -> io::Result<()> {
        self.scope.verify_current()?;
        match fs::mkdirat(&self.resources.root, "mnt", fs::Mode::from_raw_mode(0o755)) {
            Ok(()) | Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(error.into()),
        }
        let _directory = fs::openat2(
            &self.resources.root,
            "mnt",
            fs::OFlags::PATH | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC,
            fs::Mode::empty(),
            fs::ResolveFlags::BENEATH | fs::ResolveFlags::NO_SYMLINKS | fs::ResolveFlags::NO_XDEV,
        )?;
        Ok(())
    }
}
