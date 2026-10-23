// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Recipe orchestration. Unsupported execution paths fail before publication.
mod bootstrap;
mod carapace;
mod firmware;
mod rejection;
use crate::{
    cache::{Cache, CacheError},
    input::{CapturedBuild, CheckedImage, InputError, ResolvedBuild},
    inventory::{Inventory, InventoryError},
    output::{AssembledImage, AssemblyFailure, PublicationFailure},
    request::BuildRequest,
    storage::Workspace,
};

/// Terminal build diagnostics, not cleanup receipts or authority to retry a
/// build. Runtime resource ownership ends here; unfinished mounts remain until
/// disposal of the dedicated VM. Lower-level failures retain retry authority.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Cache(#[from] CacheError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Materialize(#[from] crate::context::ContextMaterializeFailure),
    #[error(transparent)]
    ReadOnly(#[from] crate::context::ContextMountFailure),
    #[error(transparent)]
    Cleanup(#[from] crate::context::ContextCleanupFailure),
    #[error(transparent)]
    Assembly(#[from] AssemblyFailure),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Publication(#[from] PublicationFailure),
    #[error(transparent)]
    Transform(#[from] crate::transform::TransformError),
    #[error("build path is not implemented yet: {0}")]
    Unsupported(&'static str),
    #[error("build execution: {0}")]
    Execution(String),
    #[error("formatter staging: {0}")]
    FormatterStaging(#[source] std::io::Error),
    #[error("formatter wait: {0}")]
    FormatterWait(#[source] crate::storage::FailureDiagnostics),
    #[error("formatter execution: {0}")]
    Formatter(#[source] crate::storage::FailureDiagnostics),
    #[error(transparent)]
    FormatterCleanup(#[from] crate::formatter::FormatterCleanupFailure),
    #[error("tool cleanup: {tool}; output cleanup: {output}")]
    StoragePairCleanup {
        #[source]
        tool: Box<BuildError>,
        output: Box<BuildError>,
    },
    #[error("build runtime: {0}")]
    RuntimeCreate(#[source] std::io::Error),
    #[error("build runtime cleanup: {0}")]
    RuntimeCleanup(#[source] std::io::Error),
    /// Both failures remain inspectable. Error::source follows the primary
    /// operation; the cleanup field exposes the independent secondary cause.
    #[error("{operation}; cleanup: {cleanup}")]
    OperationAndCleanup {
        #[source]
        operation: Box<BuildError>,
        cleanup: Box<BuildError>,
    },
    #[error("conglobate requires root")]
    RootRequired,
}
impl BuildError {
    fn with_cleanup(self, cleanup: Result<(), Self>) -> Self {
        match cleanup {
            Ok(()) => self,
            Err(cleanup) => Self::OperationAndCleanup {
                operation: Box::new(self),
                cleanup: Box::new(cleanup),
            },
        }
    }
    fn execution(error: impl std::fmt::Display) -> Self {
        Self::Execution(error.to_string())
    }
}
impl From<crate::storage::RuntimeScopeFailure<'_, BuildError>> for BuildError {
    fn from(failure: crate::storage::RuntimeScopeFailure<'_, BuildError>) -> Self {
        match failure {
            crate::storage::RuntimeScopeFailure::Operation(error) => error,
            crate::storage::RuntimeScopeFailure::Create(failure) => {
                Self::RuntimeCreate(failure.into_terminal_error())
            }
            crate::storage::RuntimeScopeFailure::Cleanup(failure) => {
                Self::RuntimeCleanup(failure.into_terminal_error())
            }
            crate::storage::RuntimeScopeFailure::Both { operation, cleanup } => {
                Self::OperationAndCleanup {
                    operation: Box::new(operation),
                    cleanup: Box::new(Self::RuntimeCleanup(cleanup.into_terminal_error())),
                }
            }
        }
    }
}
#[derive(Debug)]
pub struct Pipeline {
    request: BuildRequest,
}
impl Pipeline {
    /// Retain a parsed request without resolving inputs or acquiring resources.
    pub fn new(request: BuildRequest) -> Self {
        Self { request }
    }
    /// Resolve local inputs, execute the recipe and publish its verified output.
    /// The caller supplies the dedicated disposable-VM environment; this method
    /// does not manage VM shutdown or turn the calling process into a sandbox.
    ///
    /// # Errors
    /// Rejects non-root execution, invalid/unavailable inputs, unsupported work,
    /// failed stage execution, cleanup, assembly, inventory or publication.
    /// Ordinary errors do not authorize another build in the same VM. Some
    /// failures retain unfinished kernel resources until environment disposal;
    /// returning an error is not a successful teardown receipt.
    pub fn run(self) -> Result<(), BuildError> {
        if !rustix::process::geteuid().is_root() {
            return Err(BuildError::RootRequired);
        }
        let build =
            CapturedBuild::try_from(&self.request)?.resolve(Cache::open(self.request.cache())?)?;
        let workspace = Workspace::create()?;
        let context = workspace
            .materialize(build.captured().context())?
            .read_only()?;
        let output = if build.captured().plan().recipe().bootstrap().is_some() {
            bootstrap::BootstrapBuild::new(&build)?.execute(&self.request, &workspace)?
        } else if build.captured().plan().recipe().firmware().is_some() {
            firmware::FirmwareBuild::new(&build)?.execute(&self.request, &workspace)?
        } else if build
            .captured()
            .plan()
            .recipe()
            .carapace()
            .is_some_and(|stage| !stage.directives().is_empty())
        {
            carapace::CarapaceBuild::new(&build)?.execute(&self.request, &workspace)?
        } else {
            InheritedBuild::new(&build)?.assemble(&self.request)?
        };
        // No artifact can be published if private input cleanup fails.
        context.close()?;
        drop(workspace);
        output.publish()?;
        Ok(())
    }
}
/// Selected inherited-image path with no command/COPY/firmware work.
/// DTB transformations still execute before inventory and publication.
#[derive(Debug)]
pub(crate) struct InheritedBuild<'a> {
    build: &'a ResolvedBuild,
    base: &'a CheckedImage,
}
impl<'a> InheritedBuild<'a> {
    pub(crate) fn new(build: &'a ResolvedBuild) -> Result<Self, BuildError> {
        let recipe = build.captured().plan().recipe();
        if recipe.bootstrap().is_some() {
            return Err(BuildError::Unsupported("bootstrap coordinator"));
        }
        if recipe.firmware().is_some() {
            return Err(BuildError::Unsupported("firmware coordinator"));
        }
        let stage = recipe
            .carapace()
            .ok_or(BuildError::Unsupported("missing output base"))?;
        if !stage.directives().is_empty() {
            return Err(BuildError::Unsupported("carapace directives"));
        }
        let base = stage
            .from()
            .and_then(|name| build.images().get(name))
            .ok_or(BuildError::Unsupported("missing resolved output base"))?;
        Ok(Self { build, base })
    }
    pub(crate) fn build(&self) -> &ResolvedBuild {
        self.build
    }
    pub(crate) fn assemble(&self, request: &BuildRequest) -> Result<CompletedImage, BuildError> {
        let source = self.base.inherited_dtb(self.build.cache())?;
        let root = self.base.chain().map(|chain| hex::encode(chain.root()));
        let transformed = self
            .build
            .captured()
            .plan()
            .recipe()
            .transform()
            .apply(source.as_ref().map(|s| s.bytes()), root.as_deref())?;
        let image = AssembledImage::transformed(
            self.build.cache(),
            self.base.image(),
            request.output(),
            &transformed,
        )?;
        let inventory = Inventory::inherited(request, self, &image, &transformed)?;
        inventory.check_output(&image)?;
        Ok(CompletedImage { image, inventory })
    }
}
/// Created only after the selected recipe path completed and inventory binds
/// its finalized output. Neither decoded inventory nor raw storage can create it.
#[derive(Debug)]
pub(crate) struct CompletedImage {
    image: AssembledImage,
    inventory: Inventory,
}
impl CompletedImage {
    pub(crate) fn publish(self) -> Result<(), BuildError> {
        self.image.publish_inventory(&self.inventory)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::BuildError;
    use crate::storage::FailureDiagnostics;
    use std::io;

    #[test]
    fn formatter_rejection_retains_history_and_independent_cleanup_errors() {
        let history = FailureDiagnostics::from(io::Error::from_raw_os_error(5))
            .followed_by(io::Error::from_raw_os_error(110).into());
        let failure = BuildError::Formatter(history)
            .with_cleanup(Err(BuildError::StoragePairCleanup {
                tool: Box::new(BuildError::Io(io::Error::from_raw_os_error(16))),
                output: Box::new(BuildError::Io(io::Error::from_raw_os_error(13))),
            }))
            .with_cleanup(Err(BuildError::Io(io::Error::from_raw_os_error(28))));
        let message = failure.to_string();
        let BuildError::OperationAndCleanup { operation, cleanup } = failure else {
            panic!("runtime cleanup cause lost");
        };
        let BuildError::Io(runtime) = *cleanup else {
            panic!("runtime cause erased")
        };
        assert_eq!(runtime.raw_os_error(), Some(28));
        let BuildError::OperationAndCleanup { operation, cleanup } = *operation else {
            panic!("storage cleanup causes lost");
        };
        let BuildError::StoragePairCleanup { tool, output } = *cleanup else {
            panic!("independent storage causes erased");
        };
        for (failure, code) in [(tool, 16), (output, 13)] {
            let BuildError::Io(source) = *failure else {
                panic!("storage cause erased")
            };
            assert_eq!(source.raw_os_error(), Some(code));
        }
        let BuildError::Formatter(history) = *operation else {
            panic!("formatter cause erased");
        };
        assert_eq!(history.operation().raw_os_error(), Some(5));
        assert_eq!(history.cleanup()[0].raw_os_error(), Some(110));
        for code in [5, 110, 16, 13, 28] {
            assert!(message.contains(&io::Error::from_raw_os_error(code).to_string()));
        }
    }

    #[test]
    fn unreaped_formatter_wait_diagnostic_retains_cancellation_history() {
        let history = FailureDiagnostics::from(io::Error::from_raw_os_error(110))
            .followed_by(io::Error::from_raw_os_error(16).into());
        let failure = BuildError::FormatterWait(history.clone());
        let BuildError::FormatterWait(retained) = failure else {
            panic!("formatter wait classification lost");
        };
        assert!(std::ptr::eq(history.operation(), retained.operation()));
        assert_eq!(retained.operation().raw_os_error(), Some(110));
        assert_eq!(retained.cleanup()[0].raw_os_error(), Some(16));
    }

    #[test]
    fn successful_cleanup_does_not_wrap_formatter_failure() {
        let failure = BuildError::Formatter(io::Error::from_raw_os_error(5).into())
            .with_cleanup(Ok(()))
            .with_cleanup(Ok(()));
        let BuildError::Formatter(history) = failure else {
            panic!("successful cleanup must preserve the formatter variant");
        };
        assert_eq!(history.operation().raw_os_error(), Some(5));
        assert!(history.cleanup().is_empty());
    }
}
