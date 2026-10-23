// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Terminal stage rejection: cleanup can recover storage only for discard.
use super::BuildError;
use crate::{
    step::{DirectiveStorage, StageFailure},
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
        match self {
            StageFailure::Before {
                index,
                source,
                storage,
                ..
            } => BuildError::Directive {
                index,
                source: Box::new(BuildError::Io(source)),
            }
            .with_cleanup(storage.discard()),
            StageFailure::Preparation { index, failure } => {
                let primary = BuildError::Directive {
                    index,
                    source: Box::new(BuildError::Preparation(failure.diagnostics().clone())),
                };
                let cleanup = failure
                    .rollback()
                    .map_err(|failure| BuildError::Preparation(failure.diagnostics().clone()))
                    .and_then(|done| done.into_parts().1.discard());
                primary.with_cleanup(cleanup)
            }
            StageFailure::Copy { index, failure } => {
                let primary = BuildError::Directive {
                    index,
                    source: Box::new(BuildError::Copy(failure.diagnostics().clone())),
                };
                let cleanup = failure
                    .retry_cleanup()
                    .into_rejected_storage()
                    .map_err(|failure| BuildError::Copy(failure.diagnostics().clone()))
                    .and_then(|(_, storage, _)| storage.discard());
                primary.with_cleanup(cleanup)
            }
            StageFailure::Execution { index, failure } => {
                let primary = BuildError::execution(format!("directive {index}: {failure}"));
                let cleanup = failure
                    .retry_cleanup()
                    .into_rejected_storage()
                    .map_err(BuildError::execution)
                    .and_then(|(_, storage)| storage.discard());
                primary.with_cleanup(cleanup)
            }
            StageFailure::NetworkSetup { index, failure } => {
                let primary = BuildError::execution(format!("directive {index}: {failure}"));
                let cleanup = failure
                    .retry_cleanup()
                    .map_err(BuildError::execution)
                    .and_then(|failure| {
                        failure
                            .retry_cleanup()
                            .into_rejected_storage()
                            .map_err(BuildError::execution)
                            .and_then(|(_, storage)| storage.discard())
                    });
                primary.with_cleanup(cleanup)
            }
            StageFailure::NetworkExecution { index, failure } => {
                let primary = BuildError::execution(format!("directive {index}: {failure}"));
                let cleanup = failure
                    .retry_cleanup()
                    .into_rejected_storage()
                    .map_err(BuildError::execution)
                    .and_then(|(_, storage)| storage.discard());
                primary.with_cleanup(cleanup)
            }
        }
    }
}
