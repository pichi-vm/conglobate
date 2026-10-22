// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Supervisor-owned mutable RAM storage. Immutable layers remain cache-backed.
mod loopback;
mod verity;
pub use loopback::{LoopAttachFailure, LoopAttachment, LoopDetachFailure};
pub use verity::{VerityFailure, VerityMapping};
mod snapshot;
pub use snapshot::{SnapshotMapping, SnapshotMappingFailure, SnapshotUsage};
mod mapping;
pub use mapping::{Active, Created, Mapping, MappingFailure, MappingState};

use rustix::{fs, mount};
use std::{
    fs::{File, Metadata},
    io::{self, Read, Seek, SeekFrom, Write},
};

/// One private tmpfs for a build. The detached mount needs no globally visible
/// pathname or pathname-based unmount. Only trusted supervisor setup creates it.
/// Capacity/swap policy belongs to the environment; errors never fall back to disk.
///
/// Scratch leases cannot outlive this owner or overlap its destruction.
/// ```compile_fail
/// use conglobate::storage::Workspace;
/// fn retain_scratch_after_workspace() {
///     let workspace = Workspace::create().unwrap();
///     let mut scratch = workspace.scratch().unwrap();
///     drop(workspace);
///     std::io::Write::write_all(&mut scratch, b"late write").unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct Workspace {
    root: File,
}
impl Workspace {
    pub fn create() -> io::Result<Self> {
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
    /// Stage committed inputs on this workspace's tmpfs. The staged owner keeps
    /// an independent reference to the filesystem until its cleanup completes.
    /// This does not yet enforce a read-only mount or authorize command access.
    pub fn materialize(
        &self,
        context: &crate::context::CapturedContext,
    ) -> Result<crate::context::StagedContext, crate::context::ContextMaterializeFailure> {
        context.materialize_in(self.root.try_clone()?)
    }

    /// New anonymous scratch storage, removed on last close. Neither names nor
    /// raw descriptors escape; future kernel consumers must retain an owner, not
    /// merely claim this Rust borrow represents a device's kernel lifetime.
    pub fn scratch(&self) -> io::Result<Scratch<'_>> {
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
pub struct Scratch<'workspace> {
    file: File,
    _workspace: &'workspace Workspace,
}
impl Scratch<'_> {
    pub(crate) fn file(&self) -> &File {
        &self.file
    }
    pub fn set_len(&mut self, length: u64) -> io::Result<()> {
        self.file.set_len(length)
    }
    pub fn metadata(&self) -> io::Result<Metadata> {
        self.file.metadata()
    }
    pub fn sync_all(&self) -> io::Result<()> {
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
