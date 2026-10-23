// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{RunningCommand, StepStorage, TeardownCompletion, TeardownFailure};
use std::{io, path::Path, process::ExitStatus, time::Instant};

/// Command observation is independent of the trusted helper's eventual exit
/// status and of cleanup. An observation error (including timeout) is not success.
#[derive(Debug, thiserror::Error)]
pub enum CommandOutcome {
    #[error("command exited with {0}")]
    Exited(ExitStatus),
    #[error("command observation failed: {0}")]
    ObservationFailed(io::Error),
}
impl CommandOutcome {
    pub fn success(&self) -> bool {
        matches!(self, Self::Exited(status) if status.success())
    }
}
/// A primary outcome with its still-owned cleanup resources. This is a
/// diagnostic association, not a proof of successful image construction.
#[derive(Debug)]
pub struct CommandResult<T> {
    outcome: CommandOutcome,
    cleanup: T,
}
impl<T> CommandResult<T> {
    pub fn outcome(&self) -> &CommandOutcome {
        &self.outcome
    }
    pub fn cleanup(&self) -> &T {
        &self.cleanup
    }
    pub fn into_parts(self) -> (CommandOutcome, T) {
        (self.outcome, self.cleanup)
    }
    /// Consuming cleanup transitions preserve the original command outcome,
    /// including when the transition returns a retryable failure owner.
    pub fn map_cleanup<U>(self, transition: impl FnOnce(T) -> U) -> CommandResult<U> {
        CommandResult {
            outcome: self.outcome,
            cleanup: transition(self.cleanup),
        }
    }
}
impl<T, E> CommandResult<Result<T, E>> {
    /// Branch on cleanup success without separating either branch from the
    /// original command outcome. `?` can propagate the complete failure owner.
    pub fn transpose(self) -> Result<CommandResult<T>, CommandResult<E>> {
        match self.cleanup {
            Ok(cleanup) => Ok(CommandResult {
                outcome: self.outcome,
                cleanup,
            }),
            Err(cleanup) => Err(CommandResult {
                outcome: self.outcome,
                cleanup,
            }),
        }
    }
}
impl<'a, R: StepStorage> RunningCommand<'a, R> {
    /// Wait for the trusted helper's report under the original execution cutoff.
    /// Even on timeout or protocol failure, retain the command for full teardown.
    pub fn observe(self) -> CommandResult<Self> {
        let outcome = match self.supervision().completion_until(self.deadline()) {
            Ok(_) if Instant::now() >= self.deadline() => {
                CommandOutcome::ObservationFailed(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "command completion observed after execution deadline",
                ))
            }
            Ok(status) => CommandOutcome::Exited(status),
            Err(error) => CommandOutcome::ObservationFailed(error),
        };
        CommandResult {
            outcome,
            cleanup: self,
        }
    }
}
impl<'a, R: StepStorage> CommandResult<RunningCommand<'a, R>> {
    pub fn teardown(
        self,
        parent: &Path,
    ) -> CommandResult<Result<TeardownCompletion<R>, Box<TeardownFailure<'a, R>>>> {
        self.map_cleanup(|command| command.teardown(parent))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn cleanup_failure_and_retry_preserve_nonzero_primary_status() {
        let result = CommandResult {
            outcome: CommandOutcome::Exited(ExitStatus::from_raw(37 << 8)),
            cleanup: (),
        };
        let failed = result.map_cleanup(|()| Err::<(), _>(io::Error::from_raw_os_error(16)));
        assert!(!failed.outcome().success());
        assert_eq!(
            failed.cleanup().as_ref().unwrap_err().raw_os_error(),
            Some(16)
        );
        let retried = failed.map_cleanup(|failure| {
            assert!(failure.is_err());
            Ok::<_, io::Error>(())
        });
        let (CommandOutcome::Exited(status), cleanup) = retried.into_parts() else {
            panic!("lost primary exit status");
        };
        assert_eq!(status.code(), Some(37));
        assert!(cleanup.is_ok());
    }
    #[test]
    fn successful_cleanup_does_not_turn_timeout_into_success() {
        let result = CommandResult {
            outcome: CommandOutcome::ObservationFailed(io::ErrorKind::TimedOut.into()),
            cleanup: (),
        }
        .map_cleanup(|()| Ok::<_, io::Error>(()));
        assert!(!result.outcome().success());
        assert!(result.cleanup().is_ok());
    }
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;
    #[test]
    fn diagnostics_render_exit_code_instead_of_raw_wait_status() {
        let outcome = CommandOutcome::Exited(ExitStatus::from_raw(37 << 8));
        let message = outcome.to_string();
        assert!(message.contains("37"));
        assert!(!message.contains("9472"));
        assert!(!outcome.success());
    }
}
