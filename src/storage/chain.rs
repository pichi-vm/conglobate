// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Active, Created, Mapping, SealedScute, Workspace};
use crate::cache::{Cache, PreparedScute, VerifiedChain};
use carapace::Assemble as _;
use std::{fs::File, io, num::NonZeroU64, sync::Arc};

/// Carapace owns the verified stack. The private linear mapping preserves this
/// consumer's fallible close boundary without removing shared Carapace mappings.
/// Prepared backing remains immutable and retained for subsequent extensions.
#[derive(Debug)]
pub(crate) struct ReadOnlyChain<'s, 'w> {
    mapping: Mapping<Active>,
    _source: File,
    inputs: Vec<PreparedScute<'s, 'w>>,
    root: [u8; 32],
}

#[cfg(feature = "test-fixtures")]
impl VerifiedChain {
    pub(crate) fn assemble<'s, 'w>(
        &'s self,
        cache: &Cache,
        workspace: &'w Workspace,
    ) -> io::Result<ReadOnlyChain<'s, 'w>> {
        ReadOnlyChain::assemble(Some(self), &[], cache, workspace)
    }
}

impl<'s, 'w> ReadOnlyChain<'s, 'w> {
    /// Collect inherited and newly sealed scutes before activating once.
    pub(crate) fn assemble(
        base: Option<&'s VerifiedChain>,
        produced: &[Arc<SealedScute<'w>>],
        cache: &Cache,
        workspace: &'w Workspace,
    ) -> io::Result<Self> {
        let mut inputs = Vec::new();
        let mut root = [0; 32];
        if let Some(base) = base {
            for scute in base.scutes() {
                inputs.push(scute.prepare(cache, workspace).map_err(io::Error::other)?);
            }
            root = *base.root();
        }
        for scute in produced {
            if scute.parent() != &root {
                return Err(io::Error::other(
                    "produced scute parent differs from input chain",
                ));
            }
            inputs.push(PreparedScute::produced(Arc::clone(scute)));
            root = *scute.root();
        }
        Self::activate(inputs)
    }

    fn activate(inputs: Vec<PreparedScute<'s, 'w>>) -> io::Result<Self> {
        let root = inputs.last().map_or([0; 32], |input| *input.root());
        let scutes = inputs
            .iter()
            .map(|input| {
                let (data, hash) = input.files();
                Ok(carapace::Scute {
                    data: data.try_clone()?,
                    hash: hash.try_clone()?,
                })
            })
            .collect::<io::Result<_>>()?;
        let source = carapace::Chain {
            root: root.to_vec().into_boxed_slice(),
            scutes,
        }
        .assemble()?;
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
            root,
        })
    }

    pub(crate) fn extend(self, scute: Arc<SealedScute<'w>>) -> io::Result<Self> {
        if scute.parent() != &self.root {
            return Err(io::Error::other(
                "produced scute parent differs from active chain",
            ));
        }
        let prepared = PreparedScute::produced(scute);
        let Self {
            mapping,
            _source,
            mut inputs,
            ..
        } = self;
        // Writable consumers must have completed before replacing this view.
        mapping.close()?;
        drop(_source);
        inputs.push(prepared);
        Self::activate(inputs)
    }

    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
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

impl<'a> From<&'a ReadOnlyChain<'_, '_>> for super::SnapshotOrigin<'a> {
    fn from(chain: &'a ReadOnlyChain<'_, '_>) -> Self {
        Self::from(&chain.mapping)
    }
}
