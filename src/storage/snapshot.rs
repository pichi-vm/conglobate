// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Active, Created, LoopAttachment, Mapping};
use std::{
    io,
    num::{NonZeroU32, NonZeroU64},
};

/// Read-only origin authority borrowed from its owner. Conversion preserves the
/// owner's lifetime without exposing an assembled chain's individual mappings.
#[derive(Debug)]
pub(crate) struct SnapshotOrigin<'owner> {
    mapping: &'owner Mapping<Active>,
}
impl<'a> From<&'a Mapping<Active>> for SnapshotOrigin<'a> {
    fn from(mapping: &'a Mapping<Active>) -> Self {
        Self { mapping }
    }
}

/// Persistent writable snapshot over a read-only origin. Owned loop backing
/// prevents reuse even if deferred removal outlives this wrapper. The origin
/// borrow prevents explicit origin release while the snapshot owner is live.
/// Kernel consumers must still be quiesced before explicit close. Drop only
/// requests deferred mapping removal, not successful finalization or sealing.
#[derive(Debug)]
pub(crate) struct SnapshotMapping<'lease, 'workspace> {
    mapping: Mapping<Active>,
    _origin: SnapshotOrigin<'lease>,
    backing: LoopAttachment<'workspace>,
}
impl Mapping<Created> {
    pub(crate) fn snapshot<'a, 'w>(
        self,
        origin: impl Into<SnapshotOrigin<'a>>,
        backing: LoopAttachment<'w>,
        sectors: NonZeroU64,
    ) -> io::Result<SnapshotMapping<'a, 'w>> {
        let origin = origin.into();
        let cow = backing.id()?;
        let target = devmap_snapshot::Snapshot::from(devmap_snapshot::Parameters {
            origin: origin.mapping.id(),
            cow,
            persistence: devmap_snapshot::Persistence::Persistent(
                devmap_snapshot::Persistent::Standard,
            ),
            chunk_size: devmap_core::BlockSize::from_bytes(NonZeroU32::new(4096).unwrap())
                .expect("valid snapshot chunk size"),
            discard: devmap_snapshot::Discard::Disabled,
        });
        let mapping = self.activate(sectors, &target, false)?;
        Ok(SnapshotMapping {
            mapping,
            _origin: origin,
            backing,
        })
    }
}
impl<'a, 'w> SnapshotMapping<'a, 'w> {
    pub(super) fn open_read_only(&self) -> io::Result<std::fs::File> {
        self.mapping.open_read_only()
    }
    /// Current kernel health only. A successful query is not proof of stopped
    /// writers, final metadata flush, future validity or a sealed output layer.
    pub(crate) fn usage(&self) -> io::Result<SnapshotUsage> {
        self.mapping.snapshot_info()?.try_into()
    }

    pub(crate) fn id(&self) -> devmap_linux::DevId {
        self.mapping.id()
    }
    pub(crate) fn close(self) -> io::Result<LoopAttachment<'w>> {
        self.mapping.close()?;
        Ok(self.backing)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SnapshotUsage {
    allocated: u64,
    total: u64,
    metadata: u64,
}
impl SnapshotUsage {
    #[cfg(test)]
    pub(crate) fn allocated_sectors(self) -> u64 {
        self.allocated
    }
}
impl TryFrom<devmap_snapshot::Info> for SnapshotUsage {
    type Error = io::Error;
    fn try_from(info: devmap_snapshot::Info) -> Result<Self, Self::Error> {
        match info {
            devmap_snapshot::Info::Usage {
                allocated_sectors: allocated,
                total_sectors: total,
                metadata_sectors: metadata,
            } if total != 0 && metadata <= allocated && allocated <= total => Ok(Self {
                allocated,
                total,
                metadata,
            }),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "snapshot is invalid, exhausted, unknown, or reports inconsistent usage",
            )),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use devmap_snapshot::Info;
    #[test]
    fn exceptional_and_inconsistent_statuses_are_not_valid_usage() {
        for info in [
            Info::Invalid,
            Info::Overflow,
            Info::Unknown,
            Info::MergeFailed,
            Info::Usage {
                allocated_sectors: 3,
                total_sectors: 2,
                metadata_sectors: 1,
            },
            Info::Usage {
                allocated_sectors: 1,
                total_sectors: 2,
                metadata_sectors: 2,
            },
            Info::Usage {
                allocated_sectors: 0,
                total_sectors: 0,
                metadata_sectors: 0,
            },
        ] {
            assert_eq!(
                SnapshotUsage::try_from(info).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        let usage = SnapshotUsage::try_from(Info::Usage {
            allocated_sectors: 2,
            total_sectors: 2,
            metadata_sectors: 1,
        })
        .unwrap();
        assert_eq!(usage.allocated_sectors(), 2);
    }
}
