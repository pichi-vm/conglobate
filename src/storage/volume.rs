// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::SnapshotMapping;
use crate::{
    anchor::RecoveryDirectory,
    filesystem::{Detached, Mount},
    isolation::PrivateMountNamespace,
};
use rustix::{fs, mount};
use std::{
    fs::{File, Metadata},
    io,
    marker::PhantomData,
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::{Path, PathBuf},
    rc::Rc,
};

mod private {
    pub trait Sealed {}
}
pub trait VolumeState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub struct Unmounted;
#[derive(Debug)]
pub struct Mounted;
impl private::Sealed for Unmounted {}
impl private::Sealed for Mounted {}
impl VolumeState for Unmounted {
    type Resources = ();
}
impl VolumeState for Mounted {
    type Resources = MountedResources;
}
#[derive(Debug)]
pub struct MountedResources {
    root: File,
    lease: MountLease,
}

/// Filesystem mount ownership coupled to its writable snapshot. Unmounted is
/// not a sealing receipt or proof that no independent kernel consumer exists.
/// Root views borrow this owner; no raw root or backing descriptors escape.
/// Abandoning a mounted volume attempts a lazy detach as a destructor fallback;
/// it neither proves flush completion nor returns an Unmounted value. Use the
/// explicit fallible unmount and finalization path before sealing output.
/// ```compile_fail
/// use conglobate::storage::{Volume, Mounted};
/// fn cannot_unmount_with_view(volume: Volume<'_, '_, '_, Mounted>) {
///     let view = volume.view().unwrap();
///     let unmounted = volume.unmount().unwrap();
///     drop(view);
/// }
/// ```
/// ```compile_fail
/// use conglobate::storage::{Volume, Mounted};
/// fn cannot_release_mounted_snapshot(volume: Volume<'_, '_, '_, Mounted>) {
///     volume.into_snapshot();
/// }
/// ```
#[derive(Debug)]
pub struct Volume<'ns, 'origin, 'workspace, S: VolumeState> {
    // Mount resources must drop before the underlying snapshot.
    resources: S::Resources,
    snapshot: SnapshotMapping<'origin, 'workspace>,
    scope: Scope<'ns>,
    _thread: PhantomData<Rc<()>>,
}
#[derive(Debug)]
enum Scope<'ns> {
    Private(&'ns PrivateMountNamespace),
    Init(&'ns RecoveryDirectory),
}
impl Scope<'_> {
    fn verify_current(&self) -> io::Result<()> {
        match self {
            Self::Private(scope) => scope.verify_current(),
            Self::Init(scope) => scope.verify_current(),
        }
    }
}
/// Nonrecursive detached view cloned in the volume's namespace. Assignment to
/// a private command namespace preserves the borrow of its mounted volume.
/// ```compile_fail
/// use conglobate::{storage::{Volume, Mounted}, isolation::PrivateMountNamespace};
/// fn converted_view_retains_volume(volume: Volume<'_, '_, '_, Mounted>, scope: &PrivateMountNamespace) {
///     let view = volume.view().unwrap().in_namespace(scope).unwrap();
///     let unmounted = volume.unmount().unwrap();
///     drop(view);
/// }
/// ```
#[derive(Debug)]
pub struct VolumeView<'volume> {
    root: File,
    _volume: PhantomData<&'volume ()>,
    _thread: PhantomData<Rc<()>>,
}
impl<'volume> VolumeView<'volume> {
    pub(crate) fn descriptor(&self) -> io::Result<File> {
        self.root.try_clone()
    }
    pub fn in_namespace<'ns>(
        self,
        scope: &'ns PrivateMountNamespace,
    ) -> io::Result<Mount<'ns, Detached>>
    where
        'volume: 'ns,
    {
        Mount::from_detached(self.root, scope)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("volume mount: {source}")]
pub struct VolumeMountFailure<'ns, 'a, 'w> {
    source: io::Error,
    volume: Volume<'ns, 'a, 'w, Unmounted>,
}
impl<'ns, 'a, 'w> VolumeMountFailure<'ns, 'a, 'w> {
    pub fn into_parts(self) -> (io::Error, Volume<'ns, 'a, 'w, Unmounted>) {
        (self.source, self.volume)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("volume unmount: {source}")]
pub struct VolumeUnmountFailure<'ns, 'a, 'w> {
    source: super::FailureDiagnostics,
    pending: UnmountPending<'ns, 'a, 'w>,
}
#[derive(Debug)]
enum UnmountPending<'ns, 'a, 'w> {
    Flush(Volume<'ns, 'a, 'w, Mounted>),
    Mounted(MountLease, Volume<'ns, 'a, 'w, Unmounted>),
    Target(Point, Volume<'ns, 'a, 'w, Unmounted>),
}
impl<'ns, 'a, 'w> VolumeUnmountFailure<'ns, 'a, 'w> {
    /// Most recent failure. `diagnostics()` also retains the original operation.
    pub fn source_error(&self) -> &io::Error {
        self.source.latest()
    }
    pub fn retry(self) -> Result<Volume<'ns, 'a, 'w, Unmounted>, Self> {
        match self.pending {
            UnmountPending::Flush(volume) => volume.unmount(),
            UnmountPending::Mounted(lease, volume) => volume.release_mount(lease),
            UnmountPending::Target(point, volume) => volume.release_point(point),
        }
        .map_err(|Self { source, pending }| Self {
            source: self.source.followed_by(source),
            pending,
        })
    }
    pub fn diagnostics(&self) -> &super::FailureDiagnostics {
        &self.source
    }
}
impl<'ns, 'a, 'w> Volume<'ns, 'a, 'w, Unmounted> {
    pub fn new(snapshot: SnapshotMapping<'a, 'w>, scope: &'ns PrivateMountNamespace) -> Self {
        Self {
            resources: (),
            snapshot,
            scope: Scope::Private(scope),
            _thread: PhantomData,
        }
    }
    pub fn into_snapshot(self) -> SnapshotMapping<'a, 'w> {
        self.snapshot
    }
    /// Observe snapshot health after all filesystem writes have been flushed.
    pub(crate) fn validate_snapshot(&self) -> std::io::Result<()> {
        self.snapshot.usage().map(|_| ())
    }
    /// Mount under supervisor ownership in the verified init namespace.
    /// Command execution still requires its own private namespace and view.
    pub fn in_init_namespace(
        snapshot: SnapshotMapping<'a, 'w>,
        scope: &'ns RecoveryDirectory,
    ) -> Result<Self, VolumeMountFailure<'ns, 'a, 'w>> {
        let volume = Self {
            resources: (),
            snapshot,
            scope: Scope::Init(scope),
            _thread: PhantomData,
        };
        match volume.scope.verify_current() {
            Ok(()) => Ok(volume),
            Err(source) => Err(VolumeMountFailure { source, volume }),
        }
    }
    /// Initial backend is ext4, with fixed nodev/nosuid mount policy. The parent
    /// must be trusted tmpfs in this volume's mount namespace; it stores only the
    /// empty attachment directory, not image data. No recipe mount options pass through.
    pub fn mount_ext4(
        self,
        parent: &Path,
    ) -> Result<Volume<'ns, 'a, 'w, Mounted>, VolumeMountFailure<'ns, 'a, 'w>> {
        let attempt = (|| -> io::Result<MountedResources> {
            self.scope.verify_current()?;
            let point = Point::create(parent)?;
            let source = self.snapshot.open_read_only()?;
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
                root,
                lease: MountLease { point: Some(point) },
            })
        })();
        match attempt {
            Ok(resources) => Ok(Volume {
                resources,
                snapshot: self.snapshot,
                scope: self.scope,
                _thread: PhantomData,
            }),
            Err(source) => Err(VolumeMountFailure {
                source,
                volume: self,
            }),
        }
    }
    fn release_mount(
        self,
        mut lease: MountLease,
    ) -> Result<Self, VolumeUnmountFailure<'ns, 'a, 'w>> {
        if let Err(source) = self.scope.verify_current() {
            return Err(VolumeUnmountFailure {
                source: source.into(),
                pending: UnmountPending::Mounted(lease, self),
            });
        }
        let point = lease.point.as_ref().expect("attached mount point");
        if let Err(source) = mount::unmount(point.path(), mount::UnmountFlags::NOFOLLOW) {
            return Err(VolumeUnmountFailure {
                source: source.into(),
                pending: UnmountPending::Mounted(lease, self),
            });
        }
        self.release_point(lease.point.take().expect("unmounted mount point"))
    }
    fn release_point(self, point: Point) -> Result<Self, VolumeUnmountFailure<'ns, 'a, 'w>> {
        if let Err(source) = self.scope.verify_current() {
            return Err(VolumeUnmountFailure {
                source: source.into(),
                pending: UnmountPending::Target(point, self),
            });
        }
        match point.remove() {
            Ok(()) => Ok(self),
            Err(source) => Err(VolumeUnmountFailure {
                source: source.into(),
                pending: UnmountPending::Target(point, self),
            }),
        }
    }
}
impl<'ns, 'a, 'w> Volume<'ns, 'a, 'w, Mounted> {
    /// Copy while exclusively borrowing this volume. Outstanding root views
    /// prevent this borrow, so no command may concurrently use the filesystem.
    /// ```compile_fail
    /// use conglobate::{storage::{Volume, Mounted}, context::CapturedContext, recipe::Copy};
    /// fn no_copy_with_command_view(mut volume: Volume<'_, '_, '_, Mounted>, context: &CapturedContext, directive: &Copy) {
    ///     let view = volume.view().unwrap();
    ///     volume.copy(context, directive).unwrap();
    ///     drop(view);
    /// }
    /// ```
    pub fn copy(
        &mut self,
        context: &crate::context::CapturedContext,
        directive: &crate::recipe::Copy,
    ) -> Result<(), crate::copy::CopyError> {
        self.scope.verify_current()?;
        crate::copy::CopyDestination::new(&self.resources.root).copy(context, directive)
    }

    pub(crate) fn copy_with_output(
        &mut self,
        output: &mut Volume<'_, '_, '_, Mounted>,
        context: &crate::context::CapturedContext,
        directive: &crate::recipe::Copy,
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
        super::ownership::ImageOwnership::new(uid, gid, deadline).validate(&self.resources.root)
    }
    pub(crate) fn validate_command_root(&self) -> io::Result<()> {
        self.scope.verify_current()?;
        crate::isolation::RuntimeTargets::resolve(&self.resources.root).map(|_| ())
    }

    pub fn root_metadata(&self) -> io::Result<Metadata> {
        self.resources.root.metadata()
    }
    pub fn view(&self) -> io::Result<VolumeView<'_>> {
        self.scope.verify_current()?;
        let root = mount::open_tree(
            &self.resources.root,
            "",
            mount::OpenTreeFlags::AT_EMPTY_PATH
                | mount::OpenTreeFlags::OPEN_TREE_CLONE
                | mount::OpenTreeFlags::OPEN_TREE_CLOEXEC,
        )?;
        Ok(VolumeView {
            root: root.into(),
            _volume: PhantomData,
            _thread: PhantomData,
        })
    }
    pub fn unmount(
        self,
    ) -> Result<Volume<'ns, 'a, 'w, Unmounted>, VolumeUnmountFailure<'ns, 'a, 'w>> {
        if let Err(source) = self.scope.verify_current() {
            return Err(VolumeUnmountFailure {
                source: source.into(),
                pending: UnmountPending::Flush(self),
            });
        }
        if let Err(source) = fs::syncfs(&self.resources.root) {
            return Err(VolumeUnmountFailure {
                source: source.into(),
                pending: UnmountPending::Flush(self),
            });
        }
        let MountedResources { root, lease } = self.resources;
        drop(root);
        Volume {
            resources: (),
            snapshot: self.snapshot,
            scope: self.scope,
            _thread: PhantomData,
        }
        .release_mount(lease)
    }
}

#[derive(Debug)]
struct Point {
    parent: File,
    name: std::ffi::OsString,
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
        })
    }
    fn path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.parent.as_raw_fd())).join(&self.name)
    }
    fn remove(&self) -> io::Result<()> {
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
    /// Capture declared output files under an exclusive borrow. The coordinator
    /// separately proves command success and quiescence before this operation.
    pub fn extract_firmware<'w>(
        &mut self,
        firmware: &crate::recipe::Firmware,
        workspace: &'w super::Workspace,
        budget: &crate::step::SetupBudget,
    ) -> Result<crate::firmware::ExtractedFirmware<'w>, crate::firmware::ExtractionError> {
        self.scope.verify_current()?;
        crate::firmware::ExtractedFirmware::capture(
            &self.resources.root,
            firmware.pmi(),
            firmware.dtb(),
            workspace,
            budget,
        )
    }

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
