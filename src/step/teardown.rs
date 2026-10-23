// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::pins::{PinCleanup, PinFailure};
use super::{
    Cancelling, Draining, Finishing, Quiescent, Ready, Reaping, Running, Step, StepCompletion,
    StepFailure, StepState, StepStorage,
};
use crate::{policy::BoundCleanup, storage::FailureDiagnostics};
use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// One budget for the entire teardown, including retries. This is deliberately
/// not a recipe or CLI option. Kernel operations may outlive this budget; their
/// eventual return is checked before accepting completion.
const TEARDOWN_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
enum Phase<'a, R: StepStorage> {
    Ready(Step<'a, Ready, R>),
    Running(Step<'a, Running, R>),
    Cancelling(Step<'a, Cancelling, R>),
    Reaping(Step<'a, Reaping, R>),
    Draining(Step<'a, Draining, R>),
    Quiescent(Step<'a, Quiescent, R>),
    Finishing(Step<'a, Finishing, R>),
    Policy(StepCompletion<R>),
    PolicyRetry(Released<R>, BoundCleanup),
    Pins(Released<R>, PinCleanup),
    Complete(Released<R>),
}
#[derive(Debug)]
struct Teardown<'a, R: StepStorage> {
    phase: Phase<'a, R>,
    parent: PathBuf,
    deadline: Instant,
}
#[derive(Debug)]
struct TransitionFailure<'a, R: StepStorage> {
    source: io::Error,
    phase: Phase<'a, R>,
}
#[derive(Debug)]
pub(super) struct Released<R: StepStorage> {
    pub status: std::process::ExitStatus,
    pub reaped: crate::process::ReapedChildren,
    pub storage: R,
}
/// Policy and identity pins are gone. The original deadline still governs
/// storage finalization; no mounted storage accessor escapes this owner.
/// ```compile_fail
/// use conglobate::{step::TeardownCompletion, storage::{Volume, Mounted}};
/// fn cannot_extract_mounted_storage(completion: TeardownCompletion<Volume<'_, '_, '_, Mounted>>) {
///     completion.into_parts();
/// }
/// ```
#[derive(Debug)]
pub struct TeardownCompletion<R: StepStorage> {
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
pub struct TeardownFailure<'a, R: StepStorage> {
    diagnostics: FailureDiagnostics,
    teardown: Teardown<'a, R>,
}
impl<'a, R: StepStorage> TeardownFailure<'a, R> {
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn retry(self) -> Result<TeardownCompletion<R>, Box<Self>> {
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
    fn new(phase: Phase<'a, R>, parent: &Path) -> Self {
        Self {
            phase,
            parent: parent.to_owned(),
            deadline: Instant::now() + TEARDOWN_TIMEOUT,
        }
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
        let Self {
            phase,
            parent,
            deadline,
        } = self;
        let next = match phase {
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
            Phase::Quiescent(step) => Self::transition(
                step.finish_record(&parent).map(Phase::Finishing),
                Phase::Quiescent,
            ),
            Phase::Finishing(step) => {
                Self::transition(step.disable().map(Phase::Policy), Phase::Finishing)
            }
            Phase::Policy(completion) => {
                let (status, reaped, policy, storage) = completion.into_parts();
                let released = Released {
                    status,
                    reaped,
                    storage,
                };
                match policy.close() {
                    Ok(identities) => Ok(Phase::Pins(released, identities.into())),
                    Err(failure) => {
                        let (source, cleanup) = failure.into_parts();
                        Err(Box::new(TransitionFailure {
                            source,
                            phase: Phase::PolicyRetry(released, cleanup),
                        }))
                    }
                }
            }
            Phase::PolicyRetry(released, cleanup) => match cleanup.retry() {
                Ok(identities) => Ok(Phase::Pins(released, identities.into())),
                Err(failure) => {
                    let (source, cleanup) = failure.into_parts();
                    Err(Box::new(TransitionFailure {
                        source,
                        phase: Phase::PolicyRetry(released, cleanup),
                    }))
                }
            },
            Phase::Pins(released, cleanup) => match cleanup.run(deadline) {
                Ok(()) => Ok(Phase::Complete(released)),
                Err(failure) => {
                    let PinFailure { source, cleanup } = *failure;
                    Err(Box::new(TransitionFailure {
                        source,
                        phase: Phase::Pins(released, cleanup),
                    }))
                }
            },
            Phase::Complete(released) => return Ok(TeardownCompletion { released, deadline }),
        };
        match next {
            // A bounded number of ownership transitions. No polling or timing retries happen here.
            Ok(phase) => Self {
                phase,
                parent,
                deadline,
            }
            .run(),
            Err(failure) => {
                let TransitionFailure { source, phase } = *failure;
                Err(Self {
                    phase,
                    parent,
                    deadline,
                }
                .failed(source))
            }
        }
    }
}
impl<'a, R: StepStorage> Step<'a, Ready, R> {
    pub fn teardown(
        self,
        parent: &Path,
    ) -> Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>> {
        Teardown::new(Phase::Ready(self), parent).run()
    }
}
impl<'a, R: StepStorage> Step<'a, Running, R> {
    pub fn teardown(
        self,
        parent: &Path,
    ) -> Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>> {
        Teardown::new(Phase::Running(self), parent).run()
    }
}
impl<'a, R: StepStorage> Step<'a, Cancelling, R> {
    pub fn teardown(
        self,
        parent: &Path,
    ) -> Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>> {
        Teardown::new(Phase::Cancelling(self), parent).run()
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
