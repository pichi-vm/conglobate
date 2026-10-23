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
            .map_err(BuildError::execution)?
            .detach()
            .map_err(BuildError::execution)?;
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
        let primary = self.to_string();
        let cleanup = match self {
            StageFailure::Before { storage, .. } => storage.discard(),
            StageFailure::Preparation { failure, .. } => failure
                .rollback()
                .map_err(BuildError::execution)
                .and_then(|done| done.into_parts().1.discard()),
            StageFailure::Execution { failure, .. } => failure
                .retry_cleanup()
                .into_rejected_storage()
                .map_err(BuildError::execution)
                .and_then(|(_, storage)| storage.discard()),
            StageFailure::NetworkSetup { failure, .. } => failure
                .retry_cleanup()
                .map_err(BuildError::execution)
                .and_then(|failure| {
                    failure
                        .retry_cleanup()
                        .into_rejected_storage()
                        .map_err(BuildError::execution)
                        .and_then(|(_, storage)| storage.discard())
                }),
            StageFailure::NetworkExecution { failure, .. } => failure
                .retry_cleanup()
                .into_rejected_storage()
                .map_err(BuildError::execution)
                .and_then(|(_, storage)| storage.discard()),
            StageFailure::Copy { failure, .. } => failure
                .retry_cleanup()
                .into_rejected_storage()
                .map_err(BuildError::execution)
                .and_then(|(_, storage, _)| storage.discard()),
        };
        BuildError::execution(primary).with_cleanup(cleanup)
    }
}
