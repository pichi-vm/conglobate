// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Recipe orchestration. Unsupported execution paths fail before publication.
mod bootstrap;
mod carapace;
mod failures;
mod firmware;
mod output;
mod rejection;
use crate::{
    cache::{Cache, CacheError},
    input::{CapturedBuild, InputError, ResolvedBuild},
    inventory::{Inventory, InventoryError},
    output::{AssembledImage, AssemblyFailure, PublicationFailure},
    request::BuildRequest,
    storage::{Runtime, Workspace},
};
use output::StageOutput;

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
    InvalidExecution(&'static str),
    #[error("{0}; cleanup: complete")]
    Command(#[source] crate::step::ExecutionOutcome),
    /// Cleanup error history remains diagnostic even if a later retry succeeds.
    #[error("{outcome}; cleanup: {cleanup}")]
    CommandCleanup {
        #[source]
        outcome: crate::step::ExecutionOutcome,
        cleanup: crate::storage::FailureDiagnostics,
    },
    #[error("directive {index}: {source}")]
    Directive {
        index: usize,
        #[source]
        source: Box<BuildError>,
    },
    #[error("command preparation: {0}")]
    Preparation(#[source] crate::storage::FailureDiagnostics),
    #[error("COPY: {0}")]
    Copy(#[source] crate::storage::FailureDiagnostics),
    #[error("device mapping: {0}")]
    Mapping(#[source] crate::storage::FailureDiagnostics),
    #[error("snapshot mapping: {0}")]
    Snapshot(#[source] crate::storage::FailureDiagnostics),
    #[error("read-only chain: {0}")]
    ReadOnlyChain(#[source] crate::storage::FailureDiagnostics),
    #[error("volume mount: {0}")]
    VolumeMount(#[source] std::io::Error),
    #[error("volume unmount: {0}")]
    VolumeUnmount(#[source] crate::storage::FailureDiagnostics),
    #[error("loop attachment: {0}")]
    LoopAttach(#[source] std::io::Error),
    #[error("loop detachment: {0}")]
    LoopDetach(#[source] std::io::Error),
    #[error("snapshot sealing: {0}")]
    Seal(#[source] crate::chain::ChainError),
    #[error(transparent)]
    Firmware(#[from] crate::firmware::ExtractionError),
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
    fn command(
        outcome: crate::step::ExecutionOutcome,
        cleanup: Option<crate::storage::FailureDiagnostics>,
    ) -> Self {
        match cleanup {
            Some(cleanup) => Self::CommandCleanup { outcome, cleanup },
            None => Self::Command(outcome),
        }
    }
    fn at_directive(self, index: usize) -> Self {
        Self::Directive {
            index,
            source: Box::new(self),
        }
    }
    fn with_cleanup(self, cleanup: Result<(), Self>) -> Self {
        match cleanup {
            Ok(()) => self,
            Err(cleanup) => Self::OperationAndCleanup {
                operation: Box::new(self),
                cleanup: Box::new(cleanup),
            },
        }
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
        let recipe = build.captured().plan().recipe();
        let inherited = recipe.bootstrap().is_none()
            && recipe.firmware().is_none()
            && recipe
                .carapace()
                .is_none_or(|stage| stage.directives().is_empty());
        let stages = if inherited {
            StageOutput::inherited(&build)?
        } else {
            Runtime::scoped(&workspace, std::path::Path::new("/run"), |runtime| {
                self.execute(&build, &workspace, runtime)
            })
            .map_err(BuildError::from)?
        };
        let output = stages.assemble(&self.request, &build)?;
        drop(workspace);
        output.publish()?;
        Ok(())
    }
    fn execute<'b, 'w>(
        &self,
        build: &'b ResolvedBuild,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
    ) -> Result<StageOutput<'b, 'w>, BuildError> {
        let recipe = build.captured().plan().recipe();
        // Output ancestry is independent of the firmware's selected tool image.
        let base = recipe
            .carapace()
            .and_then(|stage| stage.from())
            .and_then(|name| build.images().get(name));
        let (active, produced, reaper, receipt) = if recipe.bootstrap().is_some() {
            let bootstrap::Bootstrapped {
                scute,
                reaper,
                receipt,
            } = bootstrap::BootstrapBuild::new(build)?.execute(
                &self.request,
                workspace,
                runtime,
            )?;
            let active = crate::storage::ReadOnlyChain::from_root(std::sync::Arc::clone(&scute))
                .map_err(BuildError::from)?;
            (Some(active), vec![scute], reaper, Some(receipt))
        } else {
            let active = base
                .and_then(|image| image.chain())
                .map(|chain| chain.assemble(build.cache(), workspace))
                .transpose()
                .map_err(BuildError::from)?;
            (
                active,
                Vec::new(),
                crate::process::ChildReaper::install()?,
                None,
            )
        };

        let directives = recipe
            .carapace()
            .map(|stage| stage.directives())
            .unwrap_or(&[]);
        let (active, produced, reaper) = if directives.is_empty() {
            (active, produced, reaper)
        } else {
            let derived = carapace::DerivedCarapace {
                active: active.ok_or(BuildError::InvalidExecution(
                    "carapace requires a root volume",
                ))?,
                scutes: produced,
                reaper,
            }
            .derive(build, workspace, runtime, directives)?;
            (Some(derived.active), derived.scutes, derived.reaper)
        };
        let root = active.as_ref().map(|chain| *chain.root());

        let pair = if let Some(stage) = recipe.firmware() {
            let tools = if let Some(reference) = stage.from() {
                if let Some(active) = active {
                    active.close().map_err(BuildError::from)?;
                }
                build
                    .images()
                    .get(reference)
                    .and_then(|image| image.chain())
                    .ok_or(BuildError::InvalidExecution(
                        "firmware tools require a root volume",
                    ))?
                    .assemble(build.cache(), workspace)
                    .map_err(BuildError::from)?
            } else {
                active.ok_or(BuildError::InvalidExecution("missing firmware tools"))?
            };
            Some(
                firmware::FirmwareTools {
                    active: tools,
                    root,
                    reaper,
                }
                .run(build, stage, workspace, runtime)?,
            )
        } else {
            if let Some(active) = active {
                active.close().map_err(BuildError::from)?;
            }
            None
        };

        Ok(StageOutput {
            base,
            produced,
            firmware: pair,
            formatter: receipt,
        })
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
    fn standard_source_chain_reaches_command_io_causes() {
        use crate::step::{CommandOutcome, ExecutionOutcome};
        use std::error::Error;
        let outcomes = [
            ExecutionOutcome::SetupFailed(io::Error::from_raw_os_error(13)),
            ExecutionOutcome::Command(CommandOutcome::ObservationFailed(
                io::Error::from_raw_os_error(13),
            )),
        ];
        for outcome in outcomes {
            let error = BuildError::command(outcome, None).at_directive(2);
            let mut current: &(dyn Error + 'static) = &error;
            let mut code = None;
            loop {
                if let Some(cause) = current.downcast_ref::<io::Error>() {
                    code = cause.raw_os_error();
                }
                match current.source() {
                    Some(source) => current = source,
                    None => break,
                }
            }
            assert_eq!(code, Some(13), "command I/O source chain was truncated");
        }
    }

    #[test]
    fn command_rejection_retains_exit_status_and_recovered_cleanup_history() {
        use crate::step::{CommandOutcome, ExecutionOutcome};
        use std::os::unix::process::ExitStatusExt;
        for status in [0, 7 << 8, 9] {
            let outcome = ExecutionOutcome::Command(CommandOutcome::Exited(
                std::process::ExitStatus::from_raw(status),
            ));
            let history = FailureDiagnostics::from(io::Error::from_raw_os_error(16));
            let error = BuildError::command(outcome, Some(history)).at_directive(3);
            let BuildError::Directive { index, source } = error else {
                panic!("index lost")
            };
            assert_eq!(index, 3);
            let BuildError::CommandCleanup { outcome, cleanup } = *source else {
                panic!("cleanup history lost");
            };
            let ExecutionOutcome::Command(CommandOutcome::Exited(exit)) = outcome else {
                panic!("exit status lost");
            };
            assert_eq!(exit.into_raw(), status);
            assert_eq!(cleanup.operation().raw_os_error(), Some(16));
        }
    }

    #[test]
    fn command_setup_error_retains_errno_after_clean_teardown() {
        use crate::step::ExecutionOutcome;
        let error = BuildError::command(
            ExecutionOutcome::SetupFailed(io::Error::from_raw_os_error(13)),
            None,
        );
        assert!(error.to_string().ends_with("cleanup: complete"));
        let BuildError::Command(ExecutionOutcome::SetupFailed(source)) = error else {
            panic!("setup error lost");
        };
        assert_eq!(source.raw_os_error(), Some(13));
    }

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
