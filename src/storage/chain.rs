// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Active, Created, Mapping, SealedScute};
use carapace::Assemble as _;
use std::{fs::File, io, num::NonZeroU64, sync::Arc};

/// Carapace owns the shared verified snapshot stack. The linear mapping
/// preserves this consumer's fallible close boundary without removing
/// Carapace's descriptor-owned mappings.
/// Retain prepared file handles and locally produced scutes across extensions.
#[derive(Debug)]
pub(crate) struct ReadOnlyChain<'w> {
    mapping: Mapping<Active>,
    _source: File,
    inputs: carapace::Chain,
    _produced: Vec<Arc<SealedScute<'w>>>,
}

impl<'w> ReadOnlyChain<'w> {
    /// Activate discovered inputs and newly sealed scutes together.
    /// Carapace revalidates their headers and verifies every consumed data block.
    pub(crate) fn assemble(
        base: Option<carapace::Chain>,
        produced: &[Arc<SealedScute<'w>>],
    ) -> io::Result<Self> {
        let mut inputs = base.unwrap_or_else(|| carapace::Chain {
            root: Box::new([0; 32]),
            scutes: Vec::new(),
        });
        for scute in produced {
            Self::append(&mut inputs, scute)?;
        }
        Self::activate(inputs, produced.to_vec())
    }

    fn append(inputs: &mut carapace::Chain, scute: &SealedScute<'_>) -> io::Result<()> {
        if inputs.root.as_ref() != scute.parent() {
            return Err(io::Error::other(
                "produced scute parent differs from input chain",
            ));
        }
        let (data, hash) = scute.files();
        inputs.scutes.push(carapace::Scute {
            data: data.try_clone()?,
            hash: hash.try_clone()?,
        });
        inputs.root = Box::new(*scute.root());
        Ok(())
    }

    fn activate(inputs: carapace::Chain, produced: Vec<Arc<SealedScute<'w>>>) -> io::Result<Self> {
        let source = inputs.assemble()?;
        let target = devmap_linux::targets::Linear {
            device: source.metadata()?.try_into()?,
            offset: 0,
        };
        let mapping = Mapping::<Created>::create()?.activate(
            NonZeroU64::new(carapace::ZERO_SIZE_SECTORS).expect("nonzero format extent"),
            &target,
            true,
        )?;
        Ok(Self {
            mapping,
            _source: source,
            inputs,
            _produced: produced,
        })
    }

    #[cfg(feature = "test-fixtures")]
    pub(crate) fn extend(self, scute: Arc<SealedScute<'w>>) -> io::Result<Self> {
        let Self {
            mapping,
            _source,
            mut inputs,
            _produced: mut produced,
        } = self;
        Self::append(&mut inputs, &scute)?;
        // Writable consumers must have completed before replacing this view.
        mapping.close()?;
        drop(_source);
        produced.push(scute);
        Self::activate(inputs, produced)
    }

    pub(crate) fn root(&self) -> &[u8; 32] {
        self.inputs
            .root
            .as_ref()
            .try_into()
            .expect("SHA-256 chain commitment")
    }

    /// Diagnostic identity, not an independent lifetime lease.
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn id(&self) -> devmap_linux::DevId {
        self.mapping.id()
    }

    /// Confirm this consumer has no remaining holders. Shared, immutable
    /// Carapace mappings retain their ordinary descriptor-based lifetime.
    pub(crate) fn close(self) -> io::Result<()> {
        self.mapping.close()
    }
}

impl<'a> From<&'a ReadOnlyChain<'_>> for super::SnapshotOrigin<'a> {
    fn from(chain: &'a ReadOnlyChain<'_>) -> Self {
        Self::from(&chain.mapping)
    }
}
