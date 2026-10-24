// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Active, Created, Mapping, MappingFailure, VerityFailure, VerityMapping, Workspace};
use crate::cache::{Cache, VerifiedChain};
use std::{io, num::NonZeroU64};

// Carapace v1's fixed dm-zero origin: 64 GiB in 512-byte sectors.
const ORIGIN_SECTORS: u64 = 64 * 1024 * 1024 * 1024 / 512;

/// Owns the complete read-only chain; individual layers cannot be released while
/// a higher layer remains publicly usable. Drop is best effort, not quiescence.
/// ```compile_fail
/// use conglobate::{cache::{Cache, VerifiedChain}, storage::Workspace};
/// fn cannot_drop_inputs(cache: &Cache, chain: VerifiedChain, workspace: &Workspace) {
///     let active = chain.assemble(cache, workspace).unwrap();
///     drop(chain);
///     active.close().unwrap();
/// }
/// ```
/// ```compile_fail
/// use conglobate::{cache::{Cache, VerifiedChain}, storage::Workspace};
/// fn cannot_drop_workspace(cache: &Cache, chain: &VerifiedChain) {
///     let workspace = Workspace::create().unwrap();
///     let active = chain.assemble(cache, &workspace).unwrap();
///     drop(workspace);
///     active.close().unwrap();
/// }
/// ```
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
#[derive(Debug)]
enum Cleanup<'s, 'w> {
    Empty,
    Stack(Stack<'s, 'w>),
    Cow(VerityMapping<'s, 'w>, Stack<'s, 'w>),
    Verity(VerityFailure<'s, 'w>, Stack<'s, 'w>),
    Mapping(MappingFailure, VerityMapping<'s, 'w>, Stack<'s, 'w>),
    Base(MappingFailure),
}
#[derive(Debug, thiserror::Error)]
#[error("read-only chain: {source}")]
pub(crate) struct ReadOnlyChainFailure<'s, 'w> {
    source: super::FailureDiagnostics,
    cleanup: Box<Cleanup<'s, 'w>>,
}
impl ReadOnlyChainFailure<'_, '_> {
    pub(crate) fn source_error(&self) -> &io::Error {
        self.source.latest()
    }
    pub(crate) fn cleanup(self) -> Result<(), Self> {
        self.cleanup
            .close()
            .map_err(|Self { source, cleanup }| Self {
                source: self.source.followed_by(source),
                cleanup,
            })
    }
    pub(crate) fn diagnostics(&self) -> &super::FailureDiagnostics {
        &self.source
    }
}
impl VerifiedChain {
    pub(crate) fn assemble<'s, 'w>(
        &'s self,
        cache: &Cache,
        workspace: &'w Workspace,
    ) -> Result<ReadOnlyChain<'s, 'w>, ReadOnlyChainFailure<'s, 'w>> {
        let sectors = NonZeroU64::new(ORIGIN_SECTORS).expect("nonzero format extent");
        let base = Mapping::<Created>::create().map_err(|error| Cleanup::Empty.failure(error))?;
        let base = base.zero(sectors).map_err(|failure| {
            let source = failure.diagnostics().latest_only();
            Cleanup::Base(failure).failure(source)
        })?;
        let mut stack = Stack {
            layers: Layers::default(),
            base,
        };
        for scute in self.scutes() {
            let prepared = match scute.prepare(cache, workspace) {
                Ok(prepared) => prepared,
                Err(error) => return Err(Cleanup::Stack(stack).failure(io::Error::other(error))),
            };
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
    pub(crate) fn from_root(
        scute: std::sync::Arc<super::SealedScute<'w>>,
    ) -> Result<Self, ReadOnlyChainFailure<'s, 'w>> {
        if scute.parent() != &[0; 32] {
            return Err(Cleanup::Empty.failure(io::Error::other("root scute has a nonzero parent")));
        }
        let base = Mapping::<Created>::create().map_err(|error| Cleanup::Empty.failure(error))?;
        let base = base
            .zero(NonZeroU64::new(ORIGIN_SECTORS).expect("nonzero extent"))
            .map_err(|failure| {
                let source = failure.diagnostics().latest_only();
                Cleanup::Base(failure).failure(source)
            })?;
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
    pub(crate) fn extend(
        self,
        scute: std::sync::Arc<super::SealedScute<'w>>,
    ) -> Result<Self, ReadOnlyChainFailure<'s, 'w>> {
        if scute.parent() != &self.root {
            return Err(Cleanup::Stack(self.stack).failure(io::Error::other(
                "produced scute parent differs from active chain",
            )));
        }
        let root = *scute.root();
        let prepared = match crate::cache::PreparedScute::produced(scute) {
            Ok(prepared) => prepared,
            Err(source) => return Err(Cleanup::Stack(self.stack).failure(io::Error::other(source))),
        };
        Ok(Self {
            stack: self.stack.append(prepared)?,
            root,
        })
    }

    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    /// Diagnostic identity, not an independent lifetime lease.
    pub(crate) fn id(&self) -> devmap_linux::DevId {
        self.stack.id()
    }
    pub(crate) fn close(self) -> Result<(), ReadOnlyChainFailure<'s, 'w>> {
        self.stack.close()
    }
}
impl<'a> From<&'a ReadOnlyChain<'_, '_>> for super::SnapshotOrigin<'a> {
    fn from(chain: &'a ReadOnlyChain<'_, '_>) -> Self {
        Self::from(chain.stack.mapping())
    }
}
impl<'s, 'w> Stack<'s, 'w> {
    fn append(
        mut self,
        prepared: crate::cache::PreparedScute<'s, 'w>,
    ) -> Result<Self, ReadOnlyChainFailure<'s, 'w>> {
        let sectors = NonZeroU64::new(ORIGIN_SECTORS).expect("nonzero format extent");
        let cow = match prepared.activate() {
            Ok(cow) => cow,
            Err(failure) => {
                let source = failure.diagnostics().latest_only();
                return Err(Cleanup::Verity(failure, self).failure(source));
            }
        };
        // Authenticate through dm-verity before the snapshot target sees it.
        let header = match cow.snapshot_header() {
            Ok(header) => header,
            Err(source) => return Err(Cleanup::Cow(cow, self).failure(source)),
        };
        let mapping = match Mapping::<Created>::create() {
            Ok(mapping) => mapping,
            Err(source) => return Err(Cleanup::Cow(cow, self).failure(source)),
        };
        let target = devmap_snapshot::Snapshot::from(devmap_snapshot::Parameters {
            origin: self.id(),
            cow: cow.id(),
            persistence: devmap_snapshot::Persistence::Persistent(
                devmap_snapshot::Persistent::Standard,
            ),
            chunk_size: header.chunk_size,
            discard: devmap_snapshot::Discard::Disabled,
        });
        match mapping.activate(sectors, &target, true) {
            Ok(mapping) => {
                if let Err(source) = mapping
                    .snapshot_info()
                    .and_then(super::SnapshotUsage::try_from)
                {
                    let failure = mapping.failed(source);
                    let source = failure.diagnostics().latest_only();
                    return Err(Cleanup::Mapping(failure, cow, self).failure(source));
                }
                self.layers.0.push(Layer { mapping, cow });
            }
            Err(failure) => {
                let source = failure.diagnostics().latest_only();
                return Err(Cleanup::Mapping(failure, cow, self).failure(source));
            }
        }
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
    fn close(mut self) -> Result<(), ReadOnlyChainFailure<'s, 'w>> {
        while let Some(Layer { mapping, cow }) = self.layers.0.pop() {
            if let Err(failure) = mapping.close() {
                let source = failure.diagnostics().latest_only();
                return Err(Cleanup::Mapping(failure, cow, self).failure(source));
            }
            if let Err(failure) = cow.close() {
                let source = failure.diagnostics().latest_only();
                return Err(Cleanup::Verity(failure, self).failure(source));
            }
        }
        self.base.close().map_err(|failure| {
            let source = failure.diagnostics().latest_only();
            Cleanup::Base(failure).failure(source)
        })
    }
}
impl<'s, 'w> Cleanup<'s, 'w> {
    fn failure(self, source: impl Into<super::FailureDiagnostics>) -> ReadOnlyChainFailure<'s, 'w> {
        ReadOnlyChainFailure {
            source: source.into(),
            cleanup: Box::new(self),
        }
    }
    fn close(self) -> Result<(), ReadOnlyChainFailure<'s, 'w>> {
        match self {
            Self::Empty => Ok(()),
            Self::Stack(stack) => stack.close(),
            Self::Cow(cow, stack) => match cow.close() {
                Ok(()) => stack.close(),
                Err(failure) => {
                    let source = failure.diagnostics().latest_only();
                    Err(Self::Verity(failure, stack).failure(source))
                }
            },
            Self::Verity(failure, stack) => match failure.cleanup() {
                Ok(()) => stack.close(),
                Err(failure) => {
                    let source = failure.diagnostics().latest_only();
                    Err(Self::Verity(failure, stack).failure(source))
                }
            },
            Self::Mapping(failure, cow, stack) => match failure.cleanup() {
                Ok(()) => Self::Cow(cow, stack).close(),
                Err(failure) => {
                    let source = failure.diagnostics().latest_only();
                    Err(Self::Mapping(failure, cow, stack).failure(source))
                }
            },
            Self::Base(failure) => failure.cleanup().map_err(|failure| {
                let source = failure.diagnostics().latest_only();
                Self::Base(failure).failure(source)
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cleanup;
    use crate::{pipeline::BuildError, storage::FailureDiagnostics};
    use std::io;

    #[test]
    fn terminal_chain_conversion_preserves_original_and_cleanup_causes() {
        let history = FailureDiagnostics::from(io::Error::from_raw_os_error(5))
            .followed_by(io::Error::from_raw_os_error(16).into());
        let failure = Cleanup::Empty.failure(history.clone());
        let BuildError::ReadOnlyChain(retained) = BuildError::from(failure) else {
            panic!("chain failure classification erased");
        };
        assert!(std::ptr::eq(history.operation(), retained.operation()));
        assert_eq!(retained.operation().raw_os_error(), Some(5));
        assert_eq!(retained.cleanup()[0].raw_os_error(), Some(16));
    }
}
