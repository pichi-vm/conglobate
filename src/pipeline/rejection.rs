// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Terminal stage rejection: cleanup can recover storage only for discard.
use super::BuildError;
use crate::{
    step::{DirectiveStorage, ExecutionStorage, StageFailure, StorageExecutionFailure},
    storage::{BootstrapVolumes, Unmounted, Volume},
};

pub(super) trait DiscardStorage {
    fn discard(self) -> Result<(), BuildError>;
}
impl DiscardStorage for Volume<'_, '_, '_, Unmounted> {
    fn discard(self) -> Result<(), BuildError> {
        self.into_snapshot()
            .close()
            .map_err(BuildError::from)?
            .detach()
            .map_err(BuildError::from)?;
        Ok(())
    }
}
impl DiscardStorage for BootstrapVolumes<'_, '_, '_, Unmounted> {
    fn discard(self) -> Result<(), BuildError> {
        let (tool, output) = self.into_parts();
        // Attempt both closes; one failure must not suppress the other cleanup.
        let tool = tool.discard();
        let output = output.discard();
        match (tool, output) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(tool), Err(output)) => Err(BuildError::StoragePairCleanup {
                tool: Box::new(tool),
                output: Box::new(output),
            }),
        }
    }
}
impl<R: DirectiveStorage> StageFailure<'_, R> {
    pub(super) fn reject(self) -> BuildError
    where
        R::Unmounted: DiscardStorage,
    {
        let index = match &self {
            Self::Before { index, .. }
            | Self::Preparation { index, .. }
            | Self::Copy { index, .. }
            | Self::Execution { index, .. }
            | Self::NetworkExecution { index, .. } => *index,
        };
        match self {
            StageFailure::Before {
                source, storage, ..
            } => BuildError::Directive {
                index,
                source: Box::new(BuildError::Io(source)),
            }
            .with_cleanup(storage.discard()),
            StageFailure::Preparation { failure, .. } => match failure.rollback() {
                Ok(done) => {
                    let (history, storage) = done.into_parts();
                    BuildError::Preparation(history)
                        .at_directive(index)
                        .with_cleanup(storage.discard())
                }
                Err(failure) => {
                    BuildError::Preparation(failure.diagnostics().clone()).at_directive(index)
                }
            },
            StageFailure::Copy { failure, .. } => match failure
                .retry_cleanup()
                .into_rejected_storage()
            {
                Ok((history, storage, _)) => BuildError::Copy(history)
                    .at_directive(index)
                    .with_cleanup(storage.discard()),
                Err(failure) => BuildError::Copy(failure.diagnostics().clone()).at_directive(index),
            },
            StageFailure::Execution { failure, .. } => failure.reject_command(index),
            StageFailure::NetworkExecution { failure, .. } => failure.reject_command(index),
        }
    }
}

impl<R: ExecutionStorage> StorageExecutionFailure<'_, R> {
    fn reject_command(self, index: usize) -> BuildError
    where
        R::Unmounted: DiscardStorage,
    {
        let failure = self.retry_cleanup();
        // Successful storage recovery otherwise consumes and discards this
        // history. Preserve it even when the final retry succeeds.
        let history = failure.cleanup_error().cloned();
        match failure.into_rejected_storage() {
            Ok((outcome, storage)) => BuildError::command(outcome, history)
                .at_directive(index)
                .with_cleanup(storage.discard()),
            Err(failure) => {
                let (outcome, history) = failure.into_terminal_parts();
                BuildError::command(outcome, history).at_directive(index)
            }
        }
    }
}
