// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Active, Created, LoopAttachment, Mapping, MappingFailure};
use std::{
    io,
    num::{NonZeroU32, NonZeroU64},
};

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
    origin: &'lease Mapping<Active>,
    backing: LoopAttachment<'workspace>,
}
#[derive(Debug, thiserror::Error)]
#[error("snapshot mapping: {failure}")]
pub struct SnapshotMappingFailure<'lease, 'workspace> {
    failure: MappingFailure,
    origin: &'lease Mapping<Active>,
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
        origin: &'a Mapping<Active>,
        backing: LoopAttachment<'w>,
        sectors: NonZeroU64,
    ) -> Result<SnapshotMapping<'a, 'w>, SnapshotMappingFailure<'a, 'w>> {
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
            origin: origin.id(),
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
