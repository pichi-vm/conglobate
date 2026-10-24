// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Supervisor-owned mutable RAM storage. Immutable layers remain cache-backed.
mod runtime;
pub(crate) use runtime::Runtime;
mod chain;
mod ownership;
mod volume;
pub(crate) use volume::{Mounted, Unmounted, Volume, VolumeState, VolumeView};
mod loop_device;
mod loopback;
pub(crate) use chain::ReadOnlyChain;
mod verity;
pub(crate) use loopback::LoopAttachment;
pub(crate) use verity::VerityMapping;
mod snapshot;
pub(crate) use snapshot::{SnapshotMapping, SnapshotOrigin, SnapshotUsage};
mod mapping;
pub(crate) use mapping::{Active, Created, Mapping};

use rustix::{fs, mount};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
};

/// One private tmpfs for a build. The detached mount needs no globally visible
/// pathname or pathname-based unmount. Only trusted supervisor setup creates it.
/// Capacity/swap policy belongs to the environment; errors never fall back to disk.
///
/// Scratch leases cannot outlive this owner or overlap its destruction.
#[derive(Debug)]
pub(crate) struct Workspace {
    root: File,
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
        Ok(Self { root: root.into() })
    }
    /// Sparse capacity for the full logical device, with 256 exception entries
    /// per metadata chunk plus the header and terminal metadata chunk.
    pub(crate) fn snapshot_store(&self) -> io::Result<Scratch<'_>> {
        let chunks = crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 4096;
        let mut scratch = self.scratch()?;
        scratch.set_len((chunks + chunks.div_ceil(256) + 2) * 4096)?;
        Ok(scratch)
    }
    /// New anonymous scratch storage, removed on last close. Neither names nor
    /// raw descriptors escape; future kernel consumers must retain an owner, not
    /// merely claim this Rust borrow represents a device's kernel lifetime.
    pub(crate) fn scratch(&self) -> io::Result<Scratch<'_>> {
        let file = fs::openat(
            &self.root,
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

/// Writable anonymous RAM storage with ordinary file I/O. This is not a sealed
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
mod sealed;
pub(crate) use sealed::{ScuteReader, SealedScute};
