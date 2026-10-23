// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Active, Created, LoopAttachment, Mapping, MappingFailure};
use std::{
    io,
    num::{NonZeroU32, NonZeroU64},
};

/// Read-only origin authority borrowed from its owner. Conversion preserves the
/// owner's lifetime without exposing an assembled chain's individual mappings.
/// ```compile_fail
/// use conglobate::storage::{ReadOnlyChain, Mapping, Created, LoopAttachment};
/// fn cannot_close_origin(chain: ReadOnlyChain<'_, '_>, backing: LoopAttachment<'_>) {
///     let snapshot = Mapping::<Created>::create().unwrap()
///         .snapshot(&chain, backing, std::num::NonZeroU64::new(8192).unwrap()).unwrap();
///     chain.close().unwrap();
///     snapshot.close().unwrap();
/// }
/// ```
/// ```compile_fail
/// use conglobate::storage::{ReadOnlyChain, Mapping, Created, LoopAttachment};
/// fn failure_also_retains_origin(chain: ReadOnlyChain<'_, '_>, backing: LoopAttachment<'_>) {
///     let failure = Mapping::<Created>::create().unwrap()
///         .snapshot(&chain, backing, std::num::NonZeroU64::new(8192).unwrap()).unwrap_err();
///     chain.close().unwrap();
///     failure.cleanup().unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct SnapshotOrigin<'owner> {
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
/// ```compile_fail
/// use conglobate::storage::{Mapping, Created, LoopAttachment};
/// use std::num::NonZeroU64;
/// fn cannot_write_backing(mut cow: LoopAttachment<'_>, origin: Mapping<conglobate::storage::Active>) {
///     let snapshot = Mapping::<Created>::create().unwrap()
///         .snapshot(&origin, cow, NonZeroU64::new(8192).unwrap()).unwrap();
///     std::io::Write::write_all(&mut cow, b"invalid concurrent writer").unwrap();
///     snapshot.close().unwrap();
/// }
/// ```
/// ```compile_fail
/// use conglobate::storage::{Mapping, Created, Active, LoopAttachment};
/// use std::num::NonZeroU64;
/// fn cannot_share_store(mut cow: LoopAttachment<'_>, origin: Mapping<Active>) {
///     let sectors = NonZeroU64::new(8192).unwrap();
///     let first = Mapping::<Created>::create().unwrap()
///         .snapshot(&origin, cow, sectors).unwrap();
///     let second = Mapping::<Created>::create().unwrap()
///         .snapshot(&origin, cow, sectors).unwrap();
///     first.close().unwrap();
/// }
/// ```
#[derive(Debug)]
pub struct SnapshotMapping<'lease, 'workspace> {
    mapping: Mapping<Active>,
    origin: SnapshotOrigin<'lease>,
    backing: LoopAttachment<'workspace>,
}
#[derive(Debug, thiserror::Error)]
#[error("snapshot mapping: {failure}")]
pub struct SnapshotMappingFailure<'lease, 'workspace> {
    failure: MappingFailure,
    origin: SnapshotOrigin<'lease>,
    backing: LoopAttachment<'workspace>,
}
impl<'a, 'w> SnapshotMappingFailure<'a, 'w> {
    pub fn source_error(&self) -> &io::Error {
        self.failure.source_error()
    }
    pub fn cleanup(self) -> Result<LoopAttachment<'w>, Self> {
        match self.failure.cleanup() {
            Ok(()) => Ok(self.backing),
            Err(failure) => Err(Self {
                failure,
                origin: self.origin,
                backing: self.backing,
            }),
        }
    }
}
impl Mapping<Created> {
    pub fn snapshot<'a, 'w>(
        self,
        origin: impl Into<SnapshotOrigin<'a>>,
        backing: LoopAttachment<'w>,
        sectors: NonZeroU64,
    ) -> Result<SnapshotMapping<'a, 'w>, SnapshotMappingFailure<'a, 'w>> {
        let origin = origin.into();
        // Device identity is obtained from a retained block FD. Metadata failure
        // must keep even the newly created empty mapping for explicit cleanup.
        let cow = match backing.id() {
            Ok(id) => id,
            Err(source) => {
                return Err(SnapshotMappingFailure {
                    failure: self.failed(source),
                    origin,
                    backing,
                });
            }
        };
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
        match self.activate(sectors, &target, false) {
            Ok(mapping) => Ok(SnapshotMapping {
                mapping,
                origin,
                backing,
            }),
            Err(failure) => Err(SnapshotMappingFailure {
                failure,
                origin,
                backing,
            }),
        }
    }
}
impl<'a, 'w> SnapshotMapping<'a, 'w> {
    pub(super) fn open_read_only(&self) -> io::Result<std::fs::File> {
        self.mapping.open_read_only()
    }
    /// Current kernel health only. A successful query is not proof of stopped
    /// writers, final metadata flush, future validity or a sealed output layer.
    pub fn usage(&self) -> io::Result<SnapshotUsage> {
        self.mapping.snapshot_info()?.try_into()
    }

    pub fn id(&self) -> devmap_linux::DevId {
        self.mapping.id()
    }
    pub fn close(self) -> Result<LoopAttachment<'w>, SnapshotMappingFailure<'a, 'w>> {
        match self.mapping.close() {
            Ok(()) => Ok(self.backing),
            Err(failure) => Err(SnapshotMappingFailure {
                failure,
                origin: self.origin,
                backing: self.backing,
            }),
        }
    }
}

/// Validated counters from one snapshot status observation, in 512-byte sectors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotUsage {
    allocated: u64,
    total: u64,
    metadata: u64,
}
impl SnapshotUsage {
    pub fn allocated_sectors(self) -> u64 {
        self.allocated
    }
    pub fn total_sectors(self) -> u64 {
        self.total
    }
    pub fn metadata_sectors(self) -> u64 {
        self.metadata
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
