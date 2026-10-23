// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! One command lifecycle, with acceptance tied to its own finalized storage.
mod storage;
use super::{
    CommandOutcome, CommandStartFailure, Ready, SetupBudget, StartFailure, Step,
    TeardownCompletion, TeardownFailure,
};
use crate::{
    storage::{BootstrapVolumes, FailureDiagnostics, Mounted, Unmounted, Volume},
    value::CommandTimeout,
};
use std::{io, path::Path};
pub use storage::ExecutionStorage;

#[derive(Debug, thiserror::Error)]
pub enum ExecutionOutcome {
    #[error("command setup: {0}")]
    SetupFailed(io::Error),
    #[error("command outcome: {0:?}")]
    Command(CommandOutcome),
}
impl ExecutionOutcome {
    fn success(&self) -> bool {
        matches!(self, Self::Command(outcome) if outcome.success())
    }
}

/// This command exited successfully, all descendants were drained, policy and
/// pins were removed, and all assigned volumes were flushed and unmounted within
/// the original teardown deadline. No public constructor accepts an unrelated
/// command outcome or arbitrary cleanup result. This is not an image seal.
///
/// ```compile_fail
/// use conglobate::{step::ExecutedVolume, storage::{Volume, Unmounted}};
/// fn cannot_forge_receipt(volume: Volume<'_, '_, '_, Unmounted>, reaped: conglobate::process::ReapedChildren) {
///     let _ = ExecutedVolume { volume, reaped };
/// }
/// ```
#[derive(Debug)]
pub struct ExecutedStorage<R: ExecutionStorage> {
    volume: R::Unmounted,
    reaped: crate::process::ReapedChildren,
}
impl<R: ExecutionStorage> ExecutedStorage<R> {
    /// Continue the assembly line with the same exclusive supervisor reaper.
    pub fn into_parts(self) -> (R::Unmounted, crate::process::ChildReaper) {
        (self.volume, self.reaped.into_reaper())
    }
    pub fn into_storage(self) -> R::Unmounted {
        self.volume
    }
}

#[derive(Debug)]
enum Cleanup<'l, R: ExecutionStorage> {
    Teardown(Box<TeardownFailure<'l, R>>),
    Finalization(R::FinalizationFailure),
    Complete(R::Finalized),
}
/// Retains the primary setup/command outcome and any unfinished cleanup owner.
/// A retry cannot restart the command or extend any cleanup deadline.
#[derive(Debug, thiserror::Error)]
#[error("{outcome}; cleanup: {cleanup:?}")]
pub struct StorageExecutionFailure<'l, R: ExecutionStorage> {
    outcome: ExecutionOutcome,
    cleanup: Cleanup<'l, R>,
}
type ExecutionResult<'l, R> = Result<ExecutedStorage<R>, Box<StorageExecutionFailure<'l, R>>>;
type TeardownResult<'l, R> = Result<TeardownCompletion<R>, Box<TeardownFailure<'l, R>>>;

impl<'l, R: ExecutionStorage> StorageExecutionFailure<'l, R> {
    pub fn outcome(&self) -> &ExecutionOutcome {
        &self.outcome
    }
    pub fn cleanup_error(&self) -> Option<&FailureDiagnostics> {
        match &self.cleanup {
            Cleanup::Teardown(failure) => Some(failure.diagnostics()),
            Cleanup::Finalization(failure) => Some(R::finalization_diagnostics(failure)),
            Cleanup::Complete(_) => None,
        }
    }
    /// A completed cleanup still cannot turn a failed command into success.
    pub fn retry_cleanup(self) -> ExecutionResult<'l, R> {
        match self.cleanup {
            Cleanup::Teardown(failure) => Self::after_teardown(self.outcome, failure.retry()),
            Cleanup::Finalization(failure) => {
                Self::after_finalization(self.outcome, R::retry_finalization(failure))
            }
            Cleanup::Complete(volume) => Self::accept(self.outcome, volume),
        }
    }
    /// Only rejected, fully finalized storage can be recovered from failure.
    /// This does not manufacture an ExecutedVolume receipt.
    pub fn into_rejected_storage(self) -> Result<(ExecutionOutcome, R::Unmounted), Box<Self>> {
        match self.cleanup {
            Cleanup::Complete(volume) => Ok((self.outcome, R::finalized_parts(volume).1)),
            _ => Err(Box::new(self)),
        }
    }
    fn after_teardown(
        outcome: ExecutionOutcome,
        result: TeardownResult<'l, R>,
    ) -> ExecutionResult<'l, R> {
        match result {
            Ok(completion) => Self::after_finalization(outcome, R::finalize(completion)),
            Err(failure) => Err(Box::new(Self {
                outcome,
                cleanup: Cleanup::Teardown(failure),
            })),
        }
    }
    fn after_finalization(
        outcome: ExecutionOutcome,
        result: Result<R::Finalized, R::FinalizationFailure>,
    ) -> ExecutionResult<'l, R> {
        match result {
            Ok(volume) => Self::accept(outcome, volume),
            Err(failure) => Err(Box::new(Self {
                outcome,
                cleanup: Cleanup::Finalization(failure),
            })),
        }
    }
    fn accept(outcome: ExecutionOutcome, volume: R::Finalized) -> ExecutionResult<'l, R> {
        if outcome.success() {
            let (reaped, volume) = R::finalized_parts(volume);
            Ok(ExecutedStorage { volume, reaped })
        } else {
            Err(Box::new(Self {
                outcome,
                cleanup: Cleanup::Complete(volume),
            }))
        }
    }
}
impl<'l, R: ExecutionStorage> Step<'l, Ready, R> {
    /// Transfer this step's assigned volume, require restricted-leaf readiness,
    /// start under the recipe timeout, observe completion and finalize storage.
    /// Every failure after launch performs bounded cleanup before returning.
    pub fn execute(
        self,
        budget: SetupBudget,
        timeout: CommandTimeout,
        domain_parent: &Path,
    ) -> ExecutionResult<'l, R> {
        if let Err(error) = budget.check().and_then(|()| {
            self.storage.0.send(
                &self.policy.policy,
                self.resources.0.supervision(),
                None,
                budget.deadline(),
            )
        }) {
            return StorageExecutionFailure::after_teardown(
                ExecutionOutcome::SetupFailed(error),
                self.teardown(domain_parent),
            );
        }
        let prepared = match budget.await_prepared(self) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, step) = failure.into_parts();
                return StorageExecutionFailure::after_teardown(
                    ExecutionOutcome::SetupFailed(error),
                    step.teardown(domain_parent),
                );
            }
        };
        let command = match prepared.start(timeout) {
            Ok(command) => command,
            Err(failure) => {
                let (error, cleanup) = match *failure {
                    CommandStartFailure::Ready(failure)
                    | CommandStartFailure::Admission(StartFailure::Activation(failure)) => {
                        let (error, step) = failure.into_parts();
                        (error, step.teardown(domain_parent))
                    }
                    CommandStartFailure::Admission(StartFailure::Admission(failure)) => {
                        let (error, step) = failure.into_parts();
                        (error, step.teardown(domain_parent))
                    }
                    CommandStartFailure::Late { source, command } => {
                        (source, command.teardown(domain_parent))
                    }
                };
                return StorageExecutionFailure::after_teardown(
                    ExecutionOutcome::SetupFailed(error),
                    cleanup,
                );
            }
        };
        let (outcome, cleanup) = command.observe().teardown(domain_parent).into_parts();
        StorageExecutionFailure::after_teardown(ExecutionOutcome::Command(outcome), cleanup)
    }
}

/// Successful single-volume execution, retaining the concrete storage type.
pub type ExecutedVolume<'n, 'o, 'w> = ExecutedStorage<Volume<'n, 'o, 'w, Mounted>>;
pub type ExecutionFailure<'l, 'n, 'o, 'w> =
    StorageExecutionFailure<'l, Volume<'n, 'o, 'w, Mounted>>;
/// Successful bootstrap execution; both tool and output were finalized.
///
/// ```compile_fail
/// use conglobate::{step::ExecutedBootstrap, storage::{BootstrapVolumes, Unmounted}};
/// fn cannot_forge_bootstrap_receipt(volume: BootstrapVolumes<'_, '_, '_, Unmounted>, reaped: conglobate::process::ReapedChildren) {
///     let _ = ExecutedBootstrap { volume, reaped };
/// }
/// ```
pub type ExecutedBootstrap<'n, 'o, 'w> = ExecutedStorage<BootstrapVolumes<'n, 'o, 'w, Mounted>>;
pub type BootstrapExecutionFailure<'l, 'n, 'o, 'w> =
    StorageExecutionFailure<'l, BootstrapVolumes<'n, 'o, 'w, Mounted>>;
impl<'n, 'o, 'w> ExecutedVolume<'n, 'o, 'w> {
    pub fn into_volume(self) -> Volume<'n, 'o, 'w, Unmounted> {
        self.into_storage()
    }
}
impl<'l, 'n, 'o, 'w> ExecutionFailure<'l, 'n, 'o, 'w> {
    pub fn into_rejected_volume(
        self,
    ) -> Result<(ExecutionOutcome, Volume<'n, 'o, 'w, Unmounted>), Box<Self>> {
        self.into_rejected_storage()
    }
}
