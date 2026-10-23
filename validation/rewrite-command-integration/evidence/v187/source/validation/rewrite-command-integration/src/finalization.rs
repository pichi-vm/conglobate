// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    step::{CommandOutcome, CommandResult, TeardownCompletion},
    storage::{Mounted, Unmounted, Volume},
};
use std::{fs::File, io, path::Path};

/// Fixture-only distinction between aborted setup and an observed command.
#[derive(Debug)]
pub(super) enum FinalizationProbe<'ns, 'origin, 'workspace> {
    Setup(TeardownCompletion<Volume<'ns, 'origin, 'workspace, Mounted>>),
    Command(CommandResult<TeardownCompletion<Volume<'ns, 'origin, 'workspace, Mounted>>>),
}
impl<'ns, 'origin, 'workspace> FinalizationProbe<'ns, 'origin, 'workspace> {
    pub(super) fn unmount(
        self,
        point: &Path,
        mode: &str,
    ) -> Result<Volume<'ns, 'origin, 'workspace, Unmounted>, Box<dyn std::error::Error>> {
        // A benign open directory keeps the mount busy until explicitly closed.
        let holder = File::open(point)?;
        let finalized = match self {
            Self::Setup(completion) => {
                let failure = completion.unmount().unwrap_err();
                assert_eq!(
                    failure.diagnostics().operation().kind(),
                    io::ErrorKind::ResourceBusy
                );
                drop(holder);
                failure.retry().map_err(|e| e.to_string())?
            }
            Self::Command(completion) => {
                let failure = completion.unmount().unwrap_err();
                Self::check_outcome(failure.outcome(), mode);
                assert_eq!(
                    failure.cleanup().diagnostics().operation().kind(),
                    io::ErrorKind::ResourceBusy
                );
                drop(holder);
                let finalized = failure
                    .retry()
                    .map_err(|failure| format!("{:?}: {}", failure.outcome(), failure.cleanup()))?;
                Self::check_outcome(finalized.outcome(), mode);
                println!("PRIMARY_OUTCOME_THROUGH_FINALIZATION_RETRY_PASS");
                finalized.into_parts().1
            }
        };
        Ok(finalized.into_parts().2)
    }
    fn check_outcome(outcome: &CommandOutcome, mode: &str) {
        match outcome {
            CommandOutcome::Exited(status) => {
                assert_ne!(mode, "timeout");
                assert_eq!(
                    status.code(),
                    Some(if matches!(mode, "shell-failure" | "policy-cleanup") {
                        37
                    } else {
                        0
                    })
                );
            }
            CommandOutcome::ObservationFailed(error) => {
                assert_eq!(mode, "timeout");
                assert_eq!(error.kind(), io::ErrorKind::TimedOut);
            }
        }
    }
}
