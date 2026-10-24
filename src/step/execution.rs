// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! One command lifecycle, with acceptance tied to its own finalized storage.
use super::{
    CommandOutcome, CommandStartFailure, CommandStorage, Ready, SetupBudget, Step,
    TeardownCompletion,
};
use crate::value::CommandTimeout;
use std::io;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ExecutionOutcome {
    #[error("command setup: {0}")]
    SetupFailed(#[source] io::Error),
    #[error("command outcome: {0}")]
    Command(#[source] CommandOutcome),
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
#[derive(Debug)]
pub(crate) struct ExecutedStorage<R: CommandStorage> {
    volume: R::Unmounted,
    reaped: crate::process::ReapedChildren,
}
impl<R: CommandStorage> ExecutedStorage<R> {
    /// Continue the assembly line with the same exclusive supervisor reaper.
    pub(crate) fn into_parts(self) -> (R::Unmounted, crate::process::ChildReaper) {
        (self.volume, self.reaped.into_reaper())
    }
}

type ExecutionResult<R> = anyhow::Result<ExecutedStorage<R>>;
impl<R: CommandStorage> ExecutedStorage<R> {
    fn after_teardown(
        outcome: ExecutionOutcome,
        cleanup: io::Result<TeardownCompletion<R>>,
    ) -> ExecutionResult<R> {
        let cleanup = cleanup
            .map_err(anyhow::Error::from)
            .and_then(TeardownCompletion::unmount);
        match (outcome.success(), cleanup) {
            (true, Ok(finalized)) => {
                let (_, reaped, volume) = finalized.into_parts();
                Ok(Self { volume, reaped })
            }

            (false, Ok(finalized)) => {
                drop(finalized);
                Err(anyhow::Error::new(outcome).context("cleanup: complete"))
            }
            (_, Err(cleanup)) => Err(cleanup.context(outcome)),
        }
    }
}
impl<'l, R: CommandStorage> Step<'l, Ready, R> {
    pub(crate) fn reject_setup(self, error: io::Error) -> anyhow::Error {
        ExecutedStorage::after_teardown(ExecutionOutcome::SetupFailed(error), self.teardown())
            .expect_err("failed setup cannot yield an accepted command")
    }
    /// Transfer this step's assigned volume, require restricted-leaf readiness,
    /// start under the recipe timeout, observe completion and finalize storage.
    /// Every failure after launch performs bounded cleanup before returning.
    pub(crate) fn execute(
        self,
        budget: SetupBudget,
        timeout: CommandTimeout,
    ) -> ExecutionResult<R> {
        if let Err(error) = budget.check().and_then(|()| {
            self.storage.0.send(
                &self.namespace,
                self.resources.0.supervision(),
                self.network.as_ref(),
                budget.deadline(),
            )
        }) {
            return ExecutedStorage::after_teardown(
                ExecutionOutcome::SetupFailed(error),
                self.teardown(),
            );
        }
        let prepared = match budget.await_prepared(self) {
            Ok(prepared) => prepared,
            Err(failure) => {
                let (error, step) = failure.into_parts();
                return ExecutedStorage::after_teardown(
                    ExecutionOutcome::SetupFailed(error),
                    step.teardown(),
                );
            }
        };
        let command = match prepared.start(timeout) {
            Ok(command) => command,
            Err(failure) => {
                let (error, cleanup) = match *failure {
                    CommandStartFailure::Ready(failure) => {
                        let (error, step) = failure.into_parts();
                        (error, step.teardown())
                    }
                    CommandStartFailure::Late { source, command } => (source, command.teardown()),
                };
                return ExecutedStorage::after_teardown(
                    ExecutionOutcome::SetupFailed(error),
                    cleanup,
                );
            }
        };
        let (outcome, cleanup) = command.observe().teardown().into_parts();
        ExecutedStorage::after_teardown(ExecutionOutcome::Command(outcome), cleanup)
    }
}
