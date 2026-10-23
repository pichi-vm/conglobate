// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    isolation::{Mapped, UserNamespace},
    network::CommandNetwork,
    process::{Joined, ReapedChildren, Supervision},
    step::{
        BootstrapFinalizationFailure, FinalizationFailure, FinalizedBootstrap, FinalizedVolume,
        StorageLayout, TeardownCompletion,
    },
    storage::{BootstrapVolumes, FailureDiagnostics, Mounted, Volume},
};
use std::{fmt::Debug, io, time::Instant};

/// Sealed storage layouts whose transfer and finalization participate in the
/// same command acceptance protocol. Finalization verifies every snapshot.
pub trait ExecutionStorage: StorageLayout {
    type Finalized: Debug;
    type FinalizationFailure: Debug;
    #[doc(hidden)]
    fn send(
        &self,
        namespace: &UserNamespace<Mapped>,
        channel: &Supervision<Joined>,
        network: Option<&CommandNetwork>,
        deadline: Instant,
    ) -> io::Result<()>;
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
    fn send(
        &self,
        namespace: &UserNamespace<Mapped>,
        channel: &Supervision<Joined>,
        network: Option<&CommandNetwork>,
        deadline: Instant,
    ) -> io::Result<()> {
        let view = self.view()?;
        match network {
            Some(network) => channel.send_network_volume_until(
                &view,
                None,
                &self.view()?,
                namespace,
                network,
                deadline,
            ),
            None => channel.send_volume_until(&view, namespace, deadline),
        }
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
    fn send(
        &self,
        namespace: &UserNamespace<Mapped>,
        channel: &Supervision<Joined>,
        network: Option<&CommandNetwork>,
        deadline: Instant,
    ) -> io::Result<()> {
        self.tool().prepare_output_mountpoint()?;
        let root = self.tool().view()?;
        let output = self.output().view()?;
        match network {
            Some(network) => channel.send_network_volume_until(
                &root,
                Some(&output),
                &self.tool().view()?,
                namespace,
                network,
                deadline,
            ),
            None => channel.send_bootstrap_until(&root, &output, namespace, deadline),
        }
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
