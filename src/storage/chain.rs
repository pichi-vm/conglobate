// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Active, Created, Mapping, VerityMapping, Workspace};
use crate::cache::{Cache, VerifiedChain};
use std::{io, num::NonZeroU64};

// Carapace v1's fixed dm-zero origin: 64 GiB in 512-byte sectors.
const ORIGIN_SECTORS: u64 = 64 * 1024 * 1024 * 1024 / 512;

/// Owns the complete read-only chain; individual layers cannot be released while
/// a higher layer remains publicly usable. Drop is best effort, not quiescence.
#[derive(Debug)]
pub(crate) struct ReadOnlyChain<'s, 'w> {
    stack: Stack<'s, 'w>,
    root: [u8; 32],
}
#[derive(Debug)]
struct Stack<'s, 'w> {
    layers: Layers<'s, 'w>,
    base: Mapping<Active>,
}
#[derive(Debug, Default)]
struct Layers<'s, 'w>(Vec<Layer<'s, 'w>>);
#[derive(Debug)]
struct Layer<'s, 'w> {
    mapping: Mapping<Active>,
    cow: VerityMapping<'s, 'w>,
}
impl Drop for Layers<'_, '_> {
    fn drop(&mut self) {
        // Unlike Vec's default order, release owners from top toward the base.
        while let Some(layer) = self.0.pop() {
            drop(layer);
        }
    }
}
impl VerifiedChain {
    pub(crate) fn assemble<'s, 'w>(
        &'s self,
        cache: &Cache,
        workspace: &'w Workspace,
    ) -> io::Result<ReadOnlyChain<'s, 'w>> {
        let sectors = NonZeroU64::new(ORIGIN_SECTORS).expect("nonzero format extent");
        let base = Mapping::<Created>::create()?.zero(sectors)?;
        let mut stack = Stack {
            layers: Layers::default(),
            base,
        };
        for scute in self.scutes() {
            let prepared = scute.prepare(cache, workspace).map_err(io::Error::other)?;
            stack = stack.append(prepared)?;
        }

        Ok(ReadOnlyChain {
            stack,
            root: *self.root(),
        })
    }
}
impl<'s, 'w> ReadOnlyChain<'s, 'w> {
    /// Activate a newly produced zero-parent root directly, retaining its RAM
    /// backing. No temporary cache artifact or filesystem-image import is needed.
    pub(crate) fn from_root(scute: std::sync::Arc<super::SealedScute<'w>>) -> io::Result<Self> {
        if scute.parent() != &[0; 32] {
            return Err(io::Error::other("root scute has a nonzero parent"));
        }
        let base = Mapping::<Created>::create()?.zero(NonZeroU64::new(ORIGIN_SECTORS).unwrap())?;
        Self {
            stack: Stack {
                layers: Layers::default(),
                base,
            },
            root: [0; 32],
        }
        .extend(scute)
    }

    /// Consume the previous read-only chain and retain the immutable produced
    /// layer directly in RAM. Inherited layers keep their original cache backing.
    pub(crate) fn extend(self, scute: std::sync::Arc<super::SealedScute<'w>>) -> io::Result<Self> {
        if scute.parent() != &self.root {
            return Err(io::Error::other(
                "produced scute parent differs from active chain",
            ));
        }
        let root = *scute.root();
        let prepared = crate::cache::PreparedScute::produced(scute).map_err(io::Error::other)?;
        Ok(Self {
            stack: self.stack.append(prepared)?,
            root,
        })
    }

    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    /// Diagnostic identity, not an independent lifetime lease.
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn id(&self) -> devmap_linux::DevId {
        self.stack.id()
    }
    pub(crate) fn close(self) -> io::Result<()> {
        self.stack.close()
    }
}
impl<'a> From<&'a ReadOnlyChain<'_, '_>> for super::SnapshotOrigin<'a> {
    fn from(chain: &'a ReadOnlyChain<'_, '_>) -> Self {
        Self::from(chain.stack.mapping())
    }
}
impl<'s, 'w> Stack<'s, 'w> {
    fn append(mut self, prepared: crate::cache::PreparedScute<'s, 'w>) -> io::Result<Self> {
        let sectors = NonZeroU64::new(ORIGIN_SECTORS).expect("nonzero format extent");
        let cow = prepared.activate()?;
        // Authenticate before exposing the scute to the snapshot target.
        let header = cow.snapshot_header()?;
        let mapping = Mapping::<Created>::create()?;
        let target = devmap_snapshot::Snapshot::from(devmap_snapshot::Parameters {
            origin: self.id(),
            cow: cow.id(),
            persistence: devmap_snapshot::Persistence::Persistent(
                devmap_snapshot::Persistent::Standard,
            ),
            chunk_size: header.chunk_size,
            discard: devmap_snapshot::Discard::Disabled,
        });
        let mapping = mapping.activate(sectors, &target, true)?;
        super::SnapshotUsage::try_from(mapping.snapshot_info()?)?;
        self.layers.0.push(Layer { mapping, cow });
        Ok(self)
    }

    fn mapping(&self) -> &Mapping<Active> {
        self.layers
            .0
            .last()
            .map_or(&self.base, |layer| &layer.mapping)
    }
    fn id(&self) -> devmap_linux::DevId {
        self.mapping().id()
    }
    fn close(mut self) -> io::Result<()> {
        while let Some(Layer { mapping, cow }) = self.layers.0.pop() {
            mapping.close()?;
            cow.close()?;
        }
        self.base.close()
    }
}
