// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    Cancelling, Draining, Quiescent, Ready, Reaping, Running, Step, StepFailure, StepState,
    StepStorage,
};
use crate::storage::FailureDiagnostics;
use std::{
    io,
    time::{Duration, Instant},
};

/// One budget for the entire teardown, including retries. This is deliberately
/// not a recipe or CLI option. Kernel operations may outlive this budget; their
/// eventual return is checked before accepting completion.
pub(super) const TEARDOWN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
enum Phase<'a, R: StepStorage> {
    Ready(Step<'a, Ready, R>),
    Running(Step<'a, Running, R>),
    AwaitingCleanup(Step<'a, Running, R>),
    Cancelling(Step<'a, Cancelling, R>),
    Reaping(Step<'a, Reaping, R>),
    Draining(Step<'a, Draining, R>),
    Quiescent(Step<'a, Quiescent, R>),
    Complete(Released<R>),
}
#[derive(Debug)]
struct Teardown<'a, R: StepStorage> {
    phase: Phase<'a, R>,
    deadline: Instant,
}
#[derive(Debug)]
struct TransitionFailure<'a, R: StepStorage> {
    source: io::Error,
    phase: Phase<'a, R>,
}
#[derive(Debug)]
pub(super) struct Released<R: StepStorage> {
    pub(crate) status: std::process::ExitStatus,
    pub(crate) reaped: crate::process::ReapedChildren,
    pub(crate) storage: R,
}
/// Command descendants are drained and namespace attachments are released. The original deadline still governs
/// storage finalization; no mounted storage accessor escapes this owner.
/// ```compile_fail
/// use conglobate::{step::TeardownCompletion, storage::{Volume, Mounted}};
/// fn cannot_extract_mounted_storage(completion: TeardownCompletion<Volume<'_, '_, '_, Mounted>>) {
///     completion.into_parts();
/// }
/// ```
#[derive(Debug)]
pub(crate) struct TeardownCompletion<R: StepStorage> {
    pub(super) released: Released<R>,
    pub(super) deadline: Instant,
}
#[derive(Debug)]
pub(super) struct Deadline;
impl Deadline {
    pub(super) fn check(deadline: Instant) -> io::Result<()> {
        if Instant::now() >= deadline {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "fixed step teardown deadline expired",
            ))
        } else {
            Ok(())
        }
    }
}
/// Retains the exact cleanup phase and original deadline. Retry never extends
/// the budget or reopens command admission. Expiry rejects the build; unfinished
/// guards/storage remain retained under the dedicated-VM disposal contract.
/// ```compile_fail
/// use conglobate::step::TeardownFailure;
/// fn cannot_extend_budget(failure: TeardownFailure<'_, ()>) {
///     failure.retry(std::time::Instant::now());
/// }
/// ```
#[derive(Debug, thiserror::Error)]
#[error("step teardown: {diagnostics}")]
pub(crate) struct TeardownFailure<'a, R: StepStorage> {
    diagnostics: FailureDiagnostics,
    teardown: Teardown<'a, R>,
}
impl<'a, R: StepStorage> TeardownFailure<'a, R> {
    pub(crate) fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub(crate) fn retry(self) -> Result<TeardownCompletion<R>, Box<Self>> {
        self.teardown.run().map_err(|failure| {
            let Self {
                diagnostics,
                teardown,
            } = *failure;
            Box::new(Self {
                diagnostics: self.diagnostics.followed_by(diagnostics),
                teardown,
            })
        })
    }
}
impl<'a, R: StepStorage> Teardown<'a, R> {
    fn new(phase: Phase<'a, R>) -> Self {
        Self::until(phase, Instant::now() + TEARDOWN_TIMEOUT)
    }
    fn until(phase: Phase<'a, R>, deadline: Instant) -> Self {
        Self { phase, deadline }
    }
    fn failed(self, source: io::Error) -> Box<TeardownFailure<'a, R>> {
        Box::new(TeardownFailure {
            diagnostics: source.into(),
            teardown: self,
        })
    }
    fn transition<S: StepState<'a>>(
        result: Result<Phase<'a, R>, Box<StepFailure<'a, S, R>>>,
        recover: impl FnOnce(Step<'a, S, R>) -> Phase<'a, R>,
    ) -> Result<Phase<'a, R>, Box<TransitionFailure<'a, R>>> {
        result.map_err(|failure| {
            let (source, step) = failure.into_parts();
            Box::new(TransitionFailure {
                source,
                phase: recover(step),
            })
        })
    }
    fn run(self) -> Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>> {
        if let Err(source) = Deadline::check(self.deadline) {
            return Err(self.failed(source));
        }
        let Self { phase, deadline } = self;
        let next = match phase {
            Phase::AwaitingCleanup(step) => match step.supervision().cleanup_until(deadline) {
                Ok(()) => Ok(Phase::Running(step)),
                Err(source) => Err(Box::new(TransitionFailure {
                    source,
                    phase: Phase::Running(step),
                })),
            },
            Phase::Ready(step) => Self::transition(
                step.cancel_until(deadline).map(Phase::Reaping),
                Phase::Cancelling,
            ),
            Phase::Running(step) => Self::transition(
                step.cancel_until(deadline).map(Phase::Reaping),
                Phase::Cancelling,
            ),
            Phase::Cancelling(step) => Self::transition(
                step.cancel_until(deadline).map(Phase::Reaping),
                Phase::Cancelling,
            ),
            Phase::Reaping(step) => Self::transition(
                step.reap_until(deadline).map(Phase::Draining),
                Phase::Reaping,
            ),
            Phase::Draining(step) => Self::transition(
                step.drain_until(deadline).map(Phase::Quiescent),
                Phase::Draining,
            ),
            Phase::Quiescent(step) => {
                let (status, reaped, namespace, storage) = step.finish().into_parts();
                drop(namespace);
                Ok(Phase::Complete(Released {
                    status,
                    reaped,
                    storage,
                }))
            }
            Phase::Complete(released) => return Ok(TeardownCompletion { released, deadline }),
        };
        match next {
            // A bounded number of ownership transitions. No polling or timing retries happen here.
            Ok(phase) => Self { phase, deadline }.run(),
            Err(failure) => {
                let TransitionFailure { source, phase } = *failure;
                Err(Self { phase, deadline }.failed(source))
            }
        }
    }
}
impl<'a, R: StepStorage> Step<'a, Ready, R> {
    pub(crate) fn teardown(self) -> Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>> {
        Teardown::new(Phase::Ready(self)).run()
    }
}
impl<'a, R: StepStorage> Step<'a, Running, R> {
    pub(super) fn teardown_after_command(
        self,
        deadline: Instant,
    ) -> Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>> {
        Teardown::until(Phase::AwaitingCleanup(self), deadline).run()
    }

    pub(crate) fn teardown(self) -> Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>> {
        self.teardown_until(Instant::now() + TEARDOWN_TIMEOUT)
    }
    pub(super) fn teardown_until(
        self,
        deadline: Instant,
    ) -> Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>> {
        Teardown::until(Phase::Running(self), deadline).run()
    }
}
impl<'a, R: StepStorage> Step<'a, Cancelling, R> {
    pub(crate) fn teardown(self) -> Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>> {
        Teardown::new(Phase::Cancelling(self)).run()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_budget_and_expiry_are_checked_without_sleeping() {
        assert_eq!(TEARDOWN_TIMEOUT, Duration::from_secs(30));
        assert_eq!(
            Deadline::check(Instant::now()).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(Deadline::check(Instant::now() + TEARDOWN_TIMEOUT).is_ok());
    }
}
