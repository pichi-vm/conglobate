// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! One command lifecycle, with acceptance tied to its own finalized storage.
use super::{
    CommandOutcome, CommandStartFailure, FinalizationFailure, FinalizedVolume, Ready, SetupBudget,
    StartFailure, Step, TeardownCompletion, TeardownFailure,
};
use crate::{
    storage::{FailureDiagnostics, Mounted, Unmounted, Volume},
    value::CommandTimeout,
};
use std::{io, path::Path};

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
/// pins were removed, and its assigned volume was flushed and unmounted within
/// the original teardown deadline. No public constructor accepts an unrelated
/// command outcome or arbitrary cleanup result. This is not an image seal.
///
/// ```compile_fail
/// use conglobate::{step::ExecutedVolume, storage::{Volume, Unmounted}};
/// fn cannot_forge_receipt(volume: Volume<'_, '_, '_, Unmounted>) {
///     let _ = ExecutedVolume { volume };
/// }
/// ```
#[derive(Debug)]
pub struct ExecutedVolume<'n, 'o, 'w> {
    volume: Volume<'n, 'o, 'w, Unmounted>,
}
impl<'n, 'o, 'w> ExecutedVolume<'n, 'o, 'w> {
    pub fn into_volume(self) -> Volume<'n, 'o, 'w, Unmounted> {
        self.volume
    }
}

#[derive(Debug)]
enum Cleanup<'l, 'n, 'o, 'w> {
    Teardown(Box<TeardownFailure<'l, Volume<'n, 'o, 'w, Mounted>>>),
    Finalization(Box<FinalizationFailure<'n, 'o, 'w>>),
    Complete(FinalizedVolume<'n, 'o, 'w>),
}
/// Retains the primary setup/command outcome and any unfinished cleanup owner.
/// A retry cannot restart the command or extend any cleanup deadline.
#[derive(Debug, thiserror::Error)]
#[error("{outcome}; cleanup: {cleanup:?}")]
pub struct ExecutionFailure<'l, 'n, 'o, 'w> {
    outcome: ExecutionOutcome,
    cleanup: Cleanup<'l, 'n, 'o, 'w>,
}
type ExecutionResult<'l, 'n, 'o, 'w> =
    Result<ExecutedVolume<'n, 'o, 'w>, Box<ExecutionFailure<'l, 'n, 'o, 'w>>>;
type TeardownResult<'l, 'n, 'o, 'w> = Result<
    TeardownCompletion<Volume<'n, 'o, 'w, Mounted>>,
    Box<TeardownFailure<'l, Volume<'n, 'o, 'w, Mounted>>>,
>;

impl<'l, 'n, 'o, 'w> ExecutionFailure<'l, 'n, 'o, 'w> {
    pub fn outcome(&self) -> &ExecutionOutcome {
        &self.outcome
    }
    pub fn cleanup_error(&self) -> Option<&FailureDiagnostics> {
        match &self.cleanup {
            Cleanup::Teardown(failure) => Some(failure.diagnostics()),
            Cleanup::Finalization(failure) => Some(failure.diagnostics()),
            Cleanup::Complete(_) => None,
        }
    }
    /// A completed cleanup still cannot turn a failed command into success.
    pub fn retry_cleanup(self) -> ExecutionResult<'l, 'n, 'o, 'w> {
        match self.cleanup {
            Cleanup::Teardown(failure) => Self::after_teardown(self.outcome, failure.retry()),
            Cleanup::Finalization(failure) => {
                Self::after_finalization(self.outcome, failure.retry())
            }
            Cleanup::Complete(volume) => Self::accept(self.outcome, volume),
        }
    }
    /// Only rejected, fully finalized storage can be recovered from failure.
    /// This does not manufacture an ExecutedVolume receipt.
    pub fn into_rejected_volume(
        self,
    ) -> Result<(ExecutionOutcome, Volume<'n, 'o, 'w, Unmounted>), Box<Self>> {
        match self.cleanup {
            Cleanup::Complete(volume) => Ok((self.outcome, volume.into_parts().2)),
            _ => Err(Box::new(self)),
        }
    }
    fn after_teardown(
        outcome: ExecutionOutcome,
        result: TeardownResult<'l, 'n, 'o, 'w>,
    ) -> ExecutionResult<'l, 'n, 'o, 'w> {
        match result {
            Ok(completion) => Self::after_finalization(outcome, completion.unmount()),
            Err(failure) => Err(Box::new(Self {
                outcome,
                cleanup: Cleanup::Teardown(failure),
            })),
        }
    }
    fn after_finalization(
        outcome: ExecutionOutcome,
        result: Result<FinalizedVolume<'n, 'o, 'w>, Box<FinalizationFailure<'n, 'o, 'w>>>,
    ) -> ExecutionResult<'l, 'n, 'o, 'w> {
        match result {
            Ok(volume) => Self::accept(outcome, volume),
            Err(failure) => Err(Box::new(Self {
                outcome,
                cleanup: Cleanup::Finalization(failure),
            })),
        }
    }
    fn accept(
        outcome: ExecutionOutcome,
        volume: FinalizedVolume<'n, 'o, 'w>,
    ) -> ExecutionResult<'l, 'n, 'o, 'w> {
        if outcome.success() {
            Ok(ExecutedVolume {
                volume: volume.into_parts().2,
            })
        } else {
            Err(Box::new(Self {
                outcome,
                cleanup: Cleanup::Complete(volume),
            }))
        }
    }
}
impl<'l, 'n, 'o, 'w> Step<'l, Ready, Volume<'n, 'o, 'w, Mounted>> {
    /// Transfer this step's assigned volume, require restricted-leaf readiness,
    /// start under the recipe timeout, observe completion and finalize storage.
    /// Every failure after launch performs bounded cleanup before returning.
    pub fn execute(
        self,
        budget: SetupBudget,
        timeout: CommandTimeout,
        domain_parent: &Path,
    ) -> ExecutionResult<'l, 'n, 'o, 'w> {
        if let Err(error) = budget
            .check()
            .and_then(|()| self.send_volume_until(budget.deadline()))
        {
            return ExecutionFailure::after_teardown(
                ExecutionOutcome::SetupFailed(error),
                self.teardown(domain_parent),
            );
        }
        let prepared = match budget.await_prepared(self) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, step) = failure.into_parts();
                return ExecutionFailure::after_teardown(
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
                return ExecutionFailure::after_teardown(
                    ExecutionOutcome::SetupFailed(error),
                    cleanup,
                );
            }
        };
        let (outcome, cleanup) = command.observe().teardown(domain_parent).into_parts();
        ExecutionFailure::after_teardown(ExecutionOutcome::Command(outcome), cleanup)
    }
}
