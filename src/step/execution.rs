// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! One command lifecycle, with acceptance tied to its own finalized storage.
mod storage;
use super::{
    CommandOutcome, CommandStartFailure, Ready, SetupBudget, Step, TeardownCompletion,
    TeardownFailure,
};
use crate::{
    storage::{BootstrapVolumes, FailureDiagnostics, Mounted, Unmounted, Volume},
    value::CommandTimeout,
};
use std::{fmt, io, path::Path};
pub use storage::ExecutionStorage;

#[derive(Debug, thiserror::Error)]
pub enum ExecutionOutcome {
    #[error("command setup: {0}")]
    SetupFailed(io::Error),
    #[error("command outcome: {0}")]
    Command(CommandOutcome),
}
impl ExecutionOutcome {
    fn success(&self) -> bool {
        matches!(self, Self::Command(outcome) if outcome.success())
    }
}

/// This command exited successfully, all descendants were drained, namespace
/// ownership was released, and all assigned volumes were flushed and unmounted within
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
#[derive(Debug)]
pub struct StorageExecutionFailure<'l, R: ExecutionStorage> {
    outcome: ExecutionOutcome,
    cleanup: Cleanup<'l, R>,
    diagnostics: Option<FailureDiagnostics>,
}
impl<R: ExecutionStorage> fmt::Display for StorageExecutionFailure<'_, R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}; cleanup: ", self.outcome)?;
        match self.cleanup_error() {
            Some(diagnostics) => fmt::Display::fmt(diagnostics, formatter),
            None => formatter.write_str("complete"),
        }
    }
}
impl<R: ExecutionStorage> std::error::Error for StorageExecutionFailure<'_, R> {}
type ExecutionResult<'l, R> = Result<ExecutedStorage<R>, Box<StorageExecutionFailure<'l, R>>>;
type TeardownResult<'l, R> = Result<TeardownCompletion<R>, Box<TeardownFailure<'l, R>>>;

impl<'l, R: ExecutionStorage> StorageExecutionFailure<'l, R> {
    pub fn outcome(&self) -> &ExecutionOutcome {
        &self.outcome
    }
    /// Cleanup error history, retained even after a successful cleanup retry.
    pub fn cleanup_error(&self) -> Option<&FailureDiagnostics> {
        self.diagnostics.as_ref()
    }
    /// Retry resource release only. A failed lifecycle cannot become an accepted
    /// execution, even if the command succeeded and cleanup now completes.
    /// ```compile_fail
    /// use conglobate::step::{ExecutionFailure, ExecutedVolume};
    /// fn cannot_promote<'l, 'n, 'o, 'w>(failure: ExecutionFailure<'l, 'n, 'o, 'w>)
    ///     -> Result<ExecutedVolume<'n, 'o, 'w>, Box<ExecutionFailure<'l, 'n, 'o, 'w>>> {
    ///     failure.retry_cleanup()
    /// }
    /// ```
    pub fn retry_cleanup(self) -> Self {
        let Self {
            outcome,
            cleanup,
            diagnostics,
        } = self;
        let cleanup = match cleanup {
            Cleanup::Teardown(failure) => match failure.retry() {
                Ok(completion) => match R::finalize(completion) {
                    Ok(volume) => Cleanup::Complete(volume),
                    Err(failure) => Cleanup::Finalization(failure),
                },
                Err(failure) => Cleanup::Teardown(failure),
            },
            Cleanup::Finalization(failure) => match R::retry_finalization(failure) {
                Ok(volume) => Cleanup::Complete(volume),
                Err(failure) => Cleanup::Finalization(failure),
            },
            Cleanup::Complete(volume) => Cleanup::Complete(volume),
        };
        let latest = match &cleanup {
            Cleanup::Teardown(failure) => Some(failure.diagnostics().latest_only()),
            Cleanup::Finalization(failure) => {
                Some(R::finalization_diagnostics(failure).latest_only())
            }
            Cleanup::Complete(_) => None,
        };
        let diagnostics = match (diagnostics, latest) {
            (Some(history), Some(latest)) => Some(history.followed_by(latest)),
            (history, None) => history,
            (None, latest) => latest,
        };
        Self {
            outcome,
            cleanup,
            diagnostics,
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
                diagnostics: Some(failure.diagnostics().clone()),
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
                diagnostics: Some(R::finalization_diagnostics(&failure).clone()),
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
                diagnostics: None,
            }))
        }
    }
}
impl<'l, R: ExecutionStorage> Step<'l, Ready, R> {
    pub(crate) fn reject_setup(
        self,
        error: io::Error,
        domain_parent: &'l Path,
    ) -> Box<StorageExecutionFailure<'l, R>> {
        StorageExecutionFailure::after_teardown(
            ExecutionOutcome::SetupFailed(error),
            self.teardown(domain_parent),
        )
        .expect_err("failed setup cannot yield an accepted command")
    }
    /// Transfer this step's assigned volume, require restricted-leaf readiness,
    /// start under the recipe timeout, observe completion and finalize storage.
    /// Every failure after launch performs bounded cleanup before returning.
    pub fn execute(
        self,
        budget: SetupBudget,
        timeout: CommandTimeout,
        domain_parent: &'l Path,
    ) -> ExecutionResult<'l, R> {
        if let Err(error) = budget.check().and_then(|()| {
            self.storage.0.send(
                &self.namespace,
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
                    CommandStartFailure::Ready(failure) => {
                        let (error, step) = failure.into_parts();
                        (error, step.teardown(domain_parent))
                    }
                    CommandStartFailure::Admission(failure) => {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        isolation::{Mapped, UserNamespace},
        network::CommandNetwork,
        process::{Joined, ReapedChildren, Supervision},
        step::{StepStorage, StorageLayout},
    };
    use std::{os::unix::process::ExitStatusExt, time::Instant};

    #[derive(Debug)]
    struct RetryableStorage;
    impl crate::step::private::Storage for RetryableStorage {}
    impl StepStorage for RetryableStorage {}
    impl StorageLayout for RetryableStorage {
        const LABEL: &'static str = "retry fixture";
        type Unmounted = ();
        fn validate(_: &()) -> io::Result<()> {
            Ok(())
        }
    }
    impl ExecutionStorage for RetryableStorage {
        type Finalized = ();
        type FinalizationFailure = FailureDiagnostics;
        fn send(
            &self,
            _: &UserNamespace<Mapped>,
            _: &Supervision<Joined>,
            _: Option<&CommandNetwork>,
            _: Instant,
        ) -> io::Result<()> {
            unreachable!("fixture only exercises finalization recovery")
        }
        fn finalize(_: TeardownCompletion<Self>) -> Result<(), FailureDiagnostics> {
            unreachable!("fixture starts at a finalization failure")
        }
        fn retry_finalization(failure: FailureDiagnostics) -> Result<(), FailureDiagnostics> {
            if failure.operation().raw_os_error() == Some(16) {
                Err(io::Error::from_raw_os_error(5).into())
            } else {
                Ok(())
            }
        }
        fn finalization_diagnostics(failure: &FailureDiagnostics) -> &FailureDiagnostics {
            failure
        }
        fn finalized_parts(_: ()) -> (ReapedChildren, ()) {
            panic!("cleanup retry must not manufacture an accepted execution")
        }
    }

    #[test]
    fn successful_command_cannot_be_accepted_after_cleanup_failure() {
        let failure = StorageExecutionFailure::<RetryableStorage>::after_finalization(
            ExecutionOutcome::Command(CommandOutcome::Exited(std::process::ExitStatus::from_raw(
                0,
            ))),
            Err(io::Error::from_raw_os_error(5).into()),
        )
        .unwrap_err();
        let recovered = failure.retry_cleanup();
        assert!(recovered.outcome().success());
        assert!(matches!(recovered.cleanup, Cleanup::Complete(())));
        assert_eq!(
            recovered
                .cleanup_error()
                .unwrap()
                .operation()
                .raw_os_error(),
            Some(5)
        );
        let recovered = recovered.retry_cleanup();
        assert!(matches!(recovered.cleanup, Cleanup::Complete(())));
        assert_eq!(
            recovered
                .cleanup_error()
                .unwrap()
                .operation()
                .raw_os_error(),
            Some(5)
        );
    }
    #[test]
    fn cleanup_history_survives_failed_then_successful_retry() {
        let failure = StorageExecutionFailure::<RetryableStorage>::after_finalization(
            ExecutionOutcome::Command(CommandOutcome::Exited(std::process::ExitStatus::from_raw(
                0,
            ))),
            Err(io::Error::from_raw_os_error(16).into()),
        )
        .unwrap_err();
        let failure = failure.retry_cleanup();
        assert!(matches!(failure.cleanup, Cleanup::Finalization(_)));
        let recovered = failure.retry_cleanup();
        assert!(matches!(recovered.cleanup, Cleanup::Complete(())));
        let history = recovered.cleanup_error().unwrap();
        assert_eq!(history.operation().raw_os_error(), Some(16));
        assert_eq!(history.cleanup().len(), 1);
        assert_eq!(history.cleanup()[0].raw_os_error(), Some(5));
    }
}
