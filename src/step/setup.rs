// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Ready, Running, StartFailure, Step, StepFailure, StepStorage, TeardownCompletion};
use crate::{
    process::{Admitted, Supervision},
    value::CommandTimeout,
};
use std::{
    io,
    time::{Duration, Instant},
};

const SETUP_TIMEOUT: Duration = Duration::from_secs(30);
/// Start before step preparation. All setup operations share this cutoff; it is
/// neither configurable nor renewed when helper readiness is received.
#[derive(Debug)]
pub(crate) struct SetupBudget {
    deadline: Instant,
}
impl SetupBudget {
    pub(crate) fn begin() -> Self {
        Self {
            deadline: Instant::now() + SETUP_TIMEOUT,
        }
    }
    /// Export a conservative absolute cutoff for trusted helpers in this VM.
    pub(crate) fn cutoff(&self) -> io::Result<super::SetupCutoff> {
        self.check()?;
        super::SetupCutoff::from_deadline(self.deadline)
    }
    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
    pub(crate) fn check(&self) -> io::Result<()> {
        if Instant::now() >= self.deadline {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "fixed step setup deadline expired",
            ))
        } else {
            Ok(())
        }
    }
    pub(crate) fn await_prepared<'a, R: StepStorage>(
        self,
        step: Step<'a, Ready, R>,
    ) -> Result<PreparedStep<'a, R>, Box<StepFailure<'a, Ready, R>>> {
        let result = self
            .check()
            .and_then(|()| {
                step.resources
                    .0
                    .supervision()
                    .wait_prepared_until(self.deadline)
            })
            .and_then(|()| self.check());
        match result {
            Ok(()) => Ok(PreparedStep { step, budget: self }),
            Err(source) => Err(Box::new(StepFailure { source, step })),
        }
    }
}
/// The trusted leaf has finished isolation and is blocked on the start gate.
/// No execution budget has been consumed yet.
#[derive(Debug)]
pub(crate) struct PreparedStep<'a, R: StepStorage> {
    step: Step<'a, Ready, R>,
    budget: SetupBudget,
}
#[derive(Debug)]
pub(crate) struct RunningCommand<'a, R: StepStorage> {
    step: Step<'a, Running, R>,
    deadline: Instant,
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum CommandStartFailure<'a, R: StepStorage> {
    #[error("before start: {0}")]
    Ready(Box<StepFailure<'a, Ready, R>>),
    #[error("command admission: {0}")]
    Admission(StartFailure<'a, R>),
    #[error("start completed after its deadline: {source}")]
    Late {
        source: io::Error,
        command: Box<RunningCommand<'a, R>>,
    },
}
impl<'a, R: StepStorage> PreparedStep<'a, R> {
    /// Count final admission/launch overhead as execution time, after all slow
    /// sandbox setup. A delayed acknowledgment never extends either cutoff.
    pub(crate) fn start(
        self,
        timeout: CommandTimeout,
    ) -> Result<RunningCommand<'a, R>, Box<CommandStartFailure<'a, R>>> {
        if let Err(source) = self.budget.check() {
            return Err(Box::new(CommandStartFailure::Ready(Box::new(
                StepFailure {
                    source,
                    step: self.step,
                },
            ))));
        }
        let Some(deadline) = Instant::now().checked_add(timeout.duration()) else {
            return Err(Box::new(CommandStartFailure::Ready(Box::new(
                StepFailure {
                    source: io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "command deadline overflows",
                    ),
                    step: self.step,
                },
            ))));
        };
        let admission = self.budget.deadline.min(deadline);
        let step = self
            .step
            .admit_until(admission)
            .map_err(|error| Box::new(CommandStartFailure::Admission(error)))?;
        let command = RunningCommand { step, deadline };
        if Instant::now() >= admission {
            return Err(Box::new(CommandStartFailure::Late {
                source: io::Error::new(io::ErrorKind::TimedOut, "command start deadline expired"),
                command: Box::new(command),
            }));
        }
        Ok(command)
    }
}
impl<'a, R: StepStorage> RunningCommand<'a, R> {
    pub(super) fn teardown_after_command(
        self,
        deadline: Instant,
    ) -> io::Result<TeardownCompletion<R>> {
        self.step.teardown_after_command(deadline)
    }

    pub(super) fn teardown_until(self, deadline: Instant) -> io::Result<TeardownCompletion<R>> {
        self.step.teardown_until(deadline)
    }

    pub(crate) fn deadline(&self) -> Instant {
        self.deadline
    }
    pub(crate) fn supervision(&self) -> &Supervision<Admitted> {
        self.step.supervision()
    }
    pub(crate) fn teardown(self) -> io::Result<TeardownCompletion<R>> {
        self.step.teardown()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_cutoff_is_fixed_and_expiry_is_checked_without_sleep() {
        let before = Instant::now();
        let budget = SetupBudget::begin();
        assert_eq!(SETUP_TIMEOUT, Duration::from_secs(30));
        assert!(budget.deadline() >= before + SETUP_TIMEOUT);
        assert!(budget.check().is_ok());
        let expired = SetupBudget { deadline: before };
        assert_eq!(expired.check().unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
}
