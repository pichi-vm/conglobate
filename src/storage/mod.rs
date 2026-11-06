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

/// One private tmpfs for a build. The detached mount needs no globally visible
/// pathname or pathname-based unmount. Only trusted supervisor setup creates it.
/// Image scratch may use an environment-supplied backing directory instead.
/// Mount/control state always stays on the private tmpfs.
///
/// Scratch leases cannot outlive this owner or overlap its destruction.
#[derive(Debug)]
pub(crate) struct Workspace {
    root: File,
    backing: Option<File>,
}
impl Workspace {
    pub(crate) fn staging_parent(&self) -> io::Result<File> {
        self.root.try_clone()
    }

    pub(crate) fn create() -> io::Result<Self> {
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
        let backing = std::env::var_os("CONGLOBATE_STORAGE")
            .map(|path| {
                fs::open(
                    std::path::Path::new(&path),
                    fs::OFlags::RDONLY
                        | fs::OFlags::DIRECTORY
                        | fs::OFlags::CLOEXEC
                        | fs::OFlags::NOFOLLOW,
                    fs::Mode::empty(),
                )
                .map(File::from)
            })
            .transpose()?;
        Ok(Self {
            root: root.into(),
            backing,
        })
    }
    /// Bound each sparse COW store by the workspace's backing capacity, including
    /// its snapshot metadata. Logical origin size does not allocate backing.
    /// Stores share the mount budget; exhaustion still fails the build.
    pub(crate) fn snapshot_store(&self) -> io::Result<Scratch<'_>> {
        let stat = fs::fstatvfs(self.backing.as_ref().unwrap_or(&self.root))?;
        let blocks = if self.backing.is_some() {
            stat.f_bavail
        } else {
            stat.f_blocks
        };
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
            self.backing.as_ref().unwrap_or(&self.root),
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
    fn snapshot_backing_tracks_workspace_capacity_without_allocating_it() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace {
            root: File::open(directory.path()).unwrap(),
            backing: None,
        };
        let stat = fs::fstatvfs(&workspace.root).unwrap();
        let scratch = workspace.snapshot_store().unwrap();
        let metadata = scratch.file().metadata().unwrap();
        assert_eq!(metadata.len(), stat.f_blocks * stat.f_frsize / 4096 * 4096);
        assert_eq!(metadata.blocks(), 0);
    }

    #[test]
    fn mounted_backing_keeps_control_separate_and_scratch_anonymous() {
        let control = tempfile::tempdir_in("/dev/shm").unwrap();
        let backing = tempfile::tempdir().unwrap();
        let workspace = Workspace {
            root: File::open(control.path()).unwrap(),
            backing: Some(File::open(backing.path()).unwrap()),
        };
        let stat = fs::fstatvfs(workspace.backing.as_ref().unwrap()).unwrap();
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
