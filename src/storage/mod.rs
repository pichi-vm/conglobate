// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Supervisor-owned mutable storage. Immutable layers remain cache-backed.
mod partitioned;
pub(crate) use partitioned::PartitionedSnapshot;
mod runtime;
pub(crate) use runtime::Runtime;
mod chain;
mod ownership;
mod volume;
pub(crate) use volume::{Mounted, Unmounted, Volume, VolumeState, VolumeView};
mod loop_device;
mod loopback;
pub(crate) use chain::ReadOnlyChain;
pub(crate) use loopback::LoopAttachment;
mod snapshot;
pub(crate) use snapshot::{SnapshotMapping, SnapshotOrigin};
mod mapping;
pub(crate) use mapping::{Active, Created, Mapping};

use rustix::{fs, mount};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
};

/// One build's control tmpfs and anonymous scratch files in /var/tmp.
/// Scratch leases cannot outlive this owner or overlap its destruction.
#[derive(Debug)]
pub(crate) struct Workspace {
    root: File,
    backing: File,
    _directory: Option<tempfile::TempDir>,
    _parent: Option<File>,
}

/// Provision a conventional application directory without following a symlink
/// or adopting an existing directory owned by another user.
pub(super) fn application_directory(path: &std::path::Path) -> io::Result<File> {
    use std::os::unix::fs::MetadataExt;
    match fs::mkdirat(fs::CWD, path, fs::Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(rustix::io::Errno::EXIST) => {}
        Err(error) => return Err(error.into()),
    }
    let directory = File::from(fs::open(
        path,
        fs::OFlags::RDONLY | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC | fs::OFlags::NOFOLLOW,
        fs::Mode::empty(),
    )?);
    let metadata = directory.metadata()?;
    if metadata.uid() != 0 || metadata.mode() & 0o777 != 0o700 {
        return Err(io::Error::other(
            "application directory must be root-owned with mode 0700",
        ));
    }
    Ok(directory)
}

impl Workspace {
    pub(crate) fn staging_parent(&self) -> io::Result<File> {
        self.root.try_clone()
    }

    pub(crate) fn create() -> io::Result<Self> {
        use std::os::{fd::AsRawFd, unix::fs::PermissionsExt};
        let parent = application_directory(std::path::Path::new("/var/tmp/conglobate"))?;
        let directory = tempfile::Builder::new()
            .prefix("build-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))?;
        let backing = File::open(directory.path())?;
        let context = mount::fsopen("tmpfs", mount::FsOpenFlags::FSOPEN_CLOEXEC)?;
        mount::fsconfig_set_string(&context, "mode", "0700")?;
        mount::fsconfig_set_string(&context, "uid", "0")?;
        mount::fsconfig_set_string(&context, "gid", "0")?;
        mount::fsconfig_create(&context)?;
        let root = mount::fsmount(
            &context,
            mount::FsMountFlags::FSMOUNT_CLOEXEC,
            mount::MountAttrFlags::MOUNT_ATTR_NODEV
                | mount::MountAttrFlags::MOUNT_ATTR_NOSUID
                | mount::MountAttrFlags::MOUNT_ATTR_NOEXEC,
        )?;
        Ok(Self {
            root: root.into(),
            backing,
            _directory: Some(directory),
            _parent: Some(parent),
        })
    }
    /// Bound each sparse COW store by the available backing capacity.
    /// Logical origin size does not allocate backing; stores share this space.
    pub(crate) fn snapshot_store(&self) -> io::Result<Scratch<'_>> {
        let stat = fs::fstatvfs(&self.backing)?;
        let blocks = stat.f_bavail;
        let bytes = blocks
            .checked_mul(stat.f_frsize)
            .ok_or_else(|| io::Error::other("workspace capacity overflow"))?;
        let bytes = bytes / 4096 * 4096;
        // Header, first metadata chunk, and at least one data chunk.
        if bytes < 3 * 4096 {
            return Err(io::Error::other("workspace too small for a snapshot"));
        }
        let mut scratch = self.scratch()?;
        scratch.set_len(bytes)?;
        Ok(scratch)
    }
    /// New anonymous scratch storage, removed on last close. Neither names nor
    /// raw descriptors escape; future kernel consumers must retain an owner, not
    /// merely claim this Rust borrow represents a device's kernel lifetime.
    pub(crate) fn scratch(&self) -> io::Result<Scratch<'_>> {
        let file = fs::openat(
            &self.backing,
            ".",
            fs::OFlags::TMPFILE | fs::OFlags::RDWR | fs::OFlags::CLOEXEC | fs::OFlags::EXCL,
            fs::Mode::RUSR | fs::Mode::WUSR,
        )?;
        Ok(Scratch {
            file: file.into(),
            _workspace: self,
        })
    }
}

/// Writable anonymous storage with ordinary file I/O. This is not a sealed
/// layer or output receipt; dropping it discards its contents. No Clone or raw
/// file accessor permits escaping its workspace lease.
#[derive(Debug)]
pub(crate) struct Scratch<'workspace> {
    file: File,
    _workspace: &'workspace Workspace,
}
impl Scratch<'_> {
    pub(crate) fn file(&self) -> &File {
        &self.file
    }
    pub(crate) fn set_len(&mut self, length: u64) -> io::Result<()> {
        self.file.set_len(length)
    }
    pub(crate) fn sync_all(&self) -> io::Result<()> {
        self.file.sync_all()
    }
}
impl Read for Scratch<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }
}
impl Write for Scratch<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
impl Seek for Scratch<'_> {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.file.seek(offset)
    }
}

mod bootstrap;
pub(crate) use bootstrap::BootstrapVolumes;
mod compact;
mod sealed;
pub(crate) use sealed::{ScuteReader, SealedScute};

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn application_directory_rejects_symlinks_and_unprotected_entries() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("target");
        std::fs::create_dir(&target).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o777)).unwrap();
        assert!(application_directory(&target).is_err());
        let link = root.path().join("link");
        symlink(&target, &link).unwrap();
        assert!(application_directory(&link).is_err());
        let file = root.path().join("file");
        std::fs::write(&file, b"keep").unwrap();
        assert!(application_directory(&file).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), b"keep");
    }

    #[test]
    #[ignore = "requires root in a private mount namespace and cgroup2"]
    fn conventional_workspace_and_runtime_cleanup() {
        use std::os::fd::AsRawFd;
        use std::path::Path;
        let workspace = Workspace::create().unwrap();
        let directory =
            std::fs::canonicalize(workspace._directory.as_ref().unwrap().path()).unwrap();
        assert!(directory.starts_with("/var/tmp/conglobate"));
        let mut scratch = workspace.scratch().unwrap();
        scratch.write_all(b"scratch bytes").unwrap();
        assert_eq!(scratch.file().metadata().unwrap().nlink(), 0);
        assert_eq!(
            scratch.file().metadata().unwrap().dev(),
            directory.metadata().unwrap().dev()
        );
        drop(scratch);
        let parent = application_directory(Path::new("/run/conglobate")).unwrap();
        let mut runtime_directory = None;
        Runtime::scoped(&workspace, Path::new("/run/conglobate"), |runtime| {
            let paths = runtime.paths()?;
            let path = paths.volumes().parent().unwrap().to_owned();
            assert!(path.starts_with("/run/conglobate"));
            let marker = format!("/proc/self/fd/{}/helper", workspace.root.as_raw_fd());
            std::fs::write(&marker, b"runtime helper")?;
            assert_eq!(std::fs::read(path.join("helper"))?, b"runtime helper");
            std::fs::remove_file(marker)?;
            runtime_directory = Some(path);
            Ok(())
        })
        .unwrap();
        assert!(!runtime_directory.unwrap().exists());
        drop(parent);
        drop(workspace);
        assert!(!directory.exists());
    }

    #[test]
    fn snapshot_backing_tracks_workspace_capacity_without_allocating_it() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace {
            root: File::open(directory.path()).unwrap(),
            backing: File::open(directory.path()).unwrap(),
            _directory: None,
            _parent: None,
        };
        let stat = fs::fstatvfs(&workspace.root).unwrap();
        let scratch = workspace.snapshot_store().unwrap();
        let metadata = scratch.file().metadata().unwrap();
        // Concurrent tests can release space after the free-space sample.
        assert!(metadata.len() <= stat.f_blocks * stat.f_frsize);
        assert_eq!(metadata.blocks(), 0);
    }

    #[test]
    fn mounted_backing_keeps_control_separate_and_scratch_anonymous() {
        let control = tempfile::tempdir_in("/dev/shm").unwrap();
        let backing = tempfile::tempdir().unwrap();
        let workspace = Workspace {
            root: File::open(control.path()).unwrap(),
            backing: File::open(backing.path()).unwrap(),
            _directory: None,
            _parent: None,
        };
        let stat = fs::fstatvfs(&workspace.backing).unwrap();
        let mut scratch = workspace.snapshot_store().unwrap();
        assert_eq!(
            scratch.file().metadata().unwrap().dev(),
            backing.path().metadata().unwrap().dev()
        );
        assert_eq!(scratch.file().metadata().unwrap().nlink(), 0);
        // Free space can change while other tests create files on this device.
        assert!(scratch.file().metadata().unwrap().len() <= stat.f_blocks * stat.f_frsize);
        assert_eq!(scratch.file().metadata().unwrap().blocks(), 0);
        assert_eq!(
            workspace
                .staging_parent()
                .unwrap()
                .metadata()
                .unwrap()
                .ino(),
            control.path().metadata().unwrap().ino()
        );
        scratch.write_all(b"image contents").unwrap();
        scratch.seek(SeekFrom::Start(0)).unwrap();
        let mut bytes = [0; 14];
        scratch.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"image contents");
        drop(scratch);
        assert_eq!(std::fs::read_dir(backing.path()).unwrap().count(), 0);
        assert_eq!(std::fs::read_dir(control.path()).unwrap().count(), 0);
    }
}
