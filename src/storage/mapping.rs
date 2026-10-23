// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use devmap_linux::{Control, DevId, Status, Table, targets::Zero};
use std::{
    fs::File,
    io::{self, Read},
    marker::PhantomData,
    num::NonZeroU64,
};

mod private {
    pub trait Sealed {}
}
pub trait MappingState: private::Sealed + std::fmt::Debug {}
#[derive(Debug)]
pub struct Created;
#[derive(Debug)]
pub struct Active;
impl private::Sealed for Created {}
impl private::Sealed for Active {}
impl MappingState for Created {}
impl MappingState for Active {}

/// Owns a newly created mapping, never adopts/removes a preexisting name. Kernel
/// device numbers are not leases; this owner must remain until consumers stop.
/// No public raw control handle or arbitrary table-loading interface is exposed.
/// ```compile_fail
/// use conglobate::storage::{Mapping, Active};
/// use std::num::NonZeroU64;
/// fn cannot_reload(active: Mapping<Active>, sectors: NonZeroU64) {
///     active.zero(sectors);
/// }
/// ```
#[derive(Debug)]
pub struct Mapping<S: MappingState> {
    lease: MappingLease,
    _state: PhantomData<S>,
}
#[derive(Debug)]
struct MappingLease {
    control: Control,
    // Drop ownership slot, not an activation-state flag.
    id: Option<DevId>,
}
/// Cleanup-only failure. A failed activation never returns an Active mapping;
/// a failed removal retains the resource and can only retry cleanup.
#[derive(Debug, thiserror::Error)]
#[error("device mapping operation: {source}")]
pub struct MappingFailure {
    source: super::FailureDiagnostics,
    lease: MappingLease,
}
impl MappingFailure {
    /// Most recent failure. `diagnostics()` also retains the original operation.
    pub fn source_error(&self) -> &io::Error {
        self.source.latest()
    }
    pub fn cleanup(self) -> Result<(), Self> {
        self.lease.close().map_err(|Self { source, lease }| Self {
            source: self.source.followed_by(source),
            lease,
        })
    }
    pub fn diagnostics(&self) -> &super::FailureDiagnostics {
        &self.source
    }
}
impl Mapping<Created> {
    pub fn create() -> io::Result<Self> {
        let control = Control::open()?;
        let mut nonce = [0; 16];
        File::open("/dev/urandom")?.read_exact(&mut nonce)?;
        let name = format!("conglobate-{}", hex::encode(nonce));
        // Collision fails without adopting the existing mapping.
        let id = control.create(&name)?;
        Ok(Self {
            lease: MappingLease {
                control,
                id: Some(id),
            },
            _state: PhantomData,
        })
    }
    pub fn zero(self, sectors: NonZeroU64) -> Result<Mapping<Active>, MappingFailure> {
        self.activate(sectors, &Zero, true)
    }
    pub(super) fn activate<T: devmap_linux::targets::Target>(
        self,
        sectors: NonZeroU64,
        target: &T,
        read_only: bool,
    ) -> Result<Mapping<Active>, MappingFailure> {
        let loaded = (|| -> io::Result<()> {
            sectors.get().checked_mul(512).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "mapping byte extent overflows")
            })?;
            let version = self.lease.control.version(T::NAME)?;
            let table = Table::<4096>::default().add(
                devmap_core::Span::new(0, sectors.get()),
                target,
                version,
            )?;
            self.lease.control.load(self.lease.id(), table, read_only)?;
            self.lease.control.suspend(self.lease.id(), false)
        })();
        match loaded {
            Ok(()) => Ok(Mapping {
                lease: self.lease,
                _state: PhantomData,
            }),
            Err(source) => Err(MappingFailure {
                source: source.into(),
                lease: self.lease,
            }),
        }
    }
}
impl Mapping<Active> {
    pub(super) fn open_read_only(&self) -> io::Result<File> {
        use std::os::unix::fs::FileTypeExt;
        let fd = rustix::fs::open(
            format!("/dev/dm-{}", self.id().minor()),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )?;
        let file = File::from(fd);
        let metadata = file.metadata()?;
        if !metadata.file_type().is_block_device() || DevId::try_from(metadata)? != self.id() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "mapping node identity mismatch",
            ));
        }
        Ok(file)
    }
    pub(super) fn snapshot_info(&self) -> io::Result<devmap_snapshot::Info> {
        use devmap_linux::records::Records;
        let version = self.lease.control.version("snapshot")?;
        let mut rows = self.lease.control.info::<4096>(self.id())?;
        let row = rows.next().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "snapshot status has no target")
        })??;
        let info = row.parse::<devmap_snapshot::Snapshot>(version)?;
        if rows.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "snapshot status has multiple targets",
            ));
        }
        Ok(info)
    }
}
impl<S: MappingState> Mapping<S> {
    pub(super) fn failed(self, source: io::Error) -> MappingFailure {
        MappingFailure {
            source: source.into(),
            lease: self.lease,
        }
    }
    /// Diagnostic identity; copying it does not retain the mapping's lifetime.
    pub fn id(&self) -> DevId {
        self.lease.id()
    }
    pub fn status(&self) -> io::Result<Status> {
        self.lease.control.status(self.lease.id())
    }
    /// Immediate removal only. Busy consumers cause a recoverable failure,
    /// never a successful cleanup receipt or unbounded polling loop.
    pub fn close(self) -> Result<(), MappingFailure> {
        self.lease.close()
    }
}
impl MappingLease {
    fn id(&self) -> DevId {
        self.id.expect("live mapping lease")
    }
    fn close(mut self) -> Result<(), MappingFailure> {
        match self.control.remove(self.id(), false) {
            Ok(()) => {
                self.id.take();
                Ok(())
            }
            Err(source) => Err(MappingFailure {
                source: source.into(),
                lease: self,
            }),
        }
    }
}
impl Drop for MappingLease {
    fn drop(&mut self) {
        if let Some(id) = self.id.take() {
            // Best effort only. Deferred removal preserves live kernel holders;
            // it is never an explicit successful close or quiescence proof.
            if let Err(error) = self.control.remove(id, true) {
                eprintln!("device mapping retained until VM disposal: {error}");
            }
        }
    }
}
