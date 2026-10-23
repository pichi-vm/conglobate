// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    process::ReapedChildren,
    step::{
        BootstrapFinalizationFailure, FinalizationFailure, FinalizedBootstrap, FinalizedVolume,
        PreparationStorage, Ready, Step, TeardownCompletion,
    },
    storage::{BootstrapVolumes, FailureDiagnostics, Mounted, Volume},
};
use std::{fmt::Debug, io, time::Instant};

/// Sealed storage layouts whose transfer and finalization participate in the
/// same command acceptance protocol. Finalization verifies every snapshot.
pub trait ExecutionStorage: PreparationStorage {
    type Finalized: Debug;
    type FinalizationFailure: Debug;
    #[doc(hidden)]
    fn send(step: &Step<'_, Ready, Self>, deadline: Instant) -> io::Result<()>;
    #[doc(hidden)]
    fn finalize(
        completion: TeardownCompletion<Self>,
    ) -> Result<Self::Finalized, Self::FinalizationFailure>;
    #[doc(hidden)]
    fn retry_finalization(
        failure: Self::FinalizationFailure,
    ) -> Result<Self::Finalized, Self::FinalizationFailure>;
    #[doc(hidden)]
    fn finalization_diagnostics(failure: &Self::FinalizationFailure) -> &FailureDiagnostics;
    #[doc(hidden)]
    fn finalized_parts(finalized: Self::Finalized) -> (ReapedChildren, Self::Unmounted);
}
impl<'n, 'o, 'w> ExecutionStorage for Volume<'n, 'o, 'w, Mounted> {
    type Finalized = FinalizedVolume<'n, 'o, 'w>;
    type FinalizationFailure = Box<FinalizationFailure<'n, 'o, 'w>>;
    fn send(step: &Step<'_, Ready, Self>, deadline: Instant) -> io::Result<()> {
        step.send_volume_until(deadline)
    }
    fn finalize(
        completion: TeardownCompletion<Self>,
    ) -> Result<Self::Finalized, Self::FinalizationFailure> {
        completion.unmount()
    }
    fn retry_finalization(
        failure: Self::FinalizationFailure,
    ) -> Result<Self::Finalized, Self::FinalizationFailure> {
        failure.retry()
    }
    fn finalization_diagnostics(failure: &Self::FinalizationFailure) -> &FailureDiagnostics {
        failure.diagnostics()
    }
    fn finalized_parts(finalized: Self::Finalized) -> (ReapedChildren, Self::Unmounted) {
        let (_, reaped, volume) = finalized.into_parts();
        (reaped, volume)
    }
}
impl<'n, 'o, 'w> ExecutionStorage for BootstrapVolumes<'n, 'o, 'w, Mounted> {
    type Finalized = FinalizedBootstrap<'n, 'o, 'w>;
    type FinalizationFailure = Box<BootstrapFinalizationFailure<'n, 'o, 'w>>;
    fn send(step: &Step<'_, Ready, Self>, deadline: Instant) -> io::Result<()> {
        step.send_bootstrap_until(deadline)
    }
    fn finalize(
        completion: TeardownCompletion<Self>,
    ) -> Result<Self::Finalized, Self::FinalizationFailure> {
        completion.unmount()
    }
    fn retry_finalization(
        failure: Self::FinalizationFailure,
    ) -> Result<Self::Finalized, Self::FinalizationFailure> {
        failure.retry()
    }
    fn finalization_diagnostics(failure: &Self::FinalizationFailure) -> &FailureDiagnostics {
        failure.diagnostics()
    }
    fn finalized_parts(finalized: Self::Finalized) -> (ReapedChildren, Self::Unmounted) {
        let (_, reaped, volume) = finalized.into_parts();
        (reaped, volume)
    }
}
