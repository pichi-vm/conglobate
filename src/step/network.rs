// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    ExecutionStorage, Ready, Retained, Step, StepStorage, StorageLayout, TeardownCompletion,
    private, teardown::Released,
};
use crate::{
    network::{CommandNetwork, NetworkCleanupFailure},
    policy::{BoundDisabled, LabelPolicy},
    process::{ReapedChildren, Registered, Supervision},
    storage::{BootstrapVolumes, FailureDiagnostics, Mounted, Volume},
};
use std::{io, marker::PhantomData, time::Instant};

/// Storage layouts that may receive one command network. This marker is sealed
/// by StepStorage; NetworkedStorage does not implement it, preventing nesting.
pub trait NetworkEligibleStorage: ExecutionStorage {}
impl NetworkEligibleStorage for Volume<'_, '_, '_, Mounted> {}
impl NetworkEligibleStorage for BootstrapVolumes<'_, '_, '_, Mounted> {}

/// A command's storage and network move together through the step's retained
/// ownership states. Neither owner is available before descendant quiescence.
/// A second network cannot replace or nest inside the retained first one.
/// ```compile_fail
/// use conglobate::{network::CommandNetwork, step::{Step, Ready, NetworkedStorage},
///     storage::{Volume, Mounted}};
/// fn cannot_replace(step: Step<'_, Ready, NetworkedStorage<Volume<'_, '_, '_, Mounted>>>,
///                   network: CommandNetwork) {
///     step.with_network(network);
/// }
/// ```
#[derive(Debug)]
pub struct NetworkedStorage<R: NetworkEligibleStorage> {
    storage: R,
    network: CommandNetwork,
}
impl<R: NetworkEligibleStorage> private::Storage for NetworkedStorage<R> {}
impl<R: NetworkEligibleStorage> StepStorage for NetworkedStorage<R> {}
impl<R: NetworkEligibleStorage> StorageLayout for NetworkedStorage<R> {
    type Unmounted = R::Unmounted;
}
impl<'l, R: NetworkEligibleStorage> Step<'l, Ready, R> {
    /// Consume network ownership while the registered helper is blocked on its
    /// volume input. Failed setup and teardown retain it with the assigned root.
    pub fn with_network(self, network: CommandNetwork) -> Step<'l, Ready, NetworkedStorage<R>> {
        Step {
            storage: Retained::new(NetworkedStorage {
                storage: self.storage.release(),
                network,
            }),
            policy: self.policy,
            resources: self.resources,
            _lease: PhantomData,
        }
    }
}
#[derive(Debug, thiserror::Error)]
#[error("networked storage finalization: {pending:?}")]
pub struct NetworkFinalizationFailure<R: NetworkEligibleStorage> {
    pending: Pending<R>,
}
#[derive(Debug)]
enum Pending<R: NetworkEligibleStorage> {
    Network {
        failure: Box<NetworkCleanupFailure>,
        completion: TeardownCompletion<R>,
    },
    Storage(R::FinalizationFailure),
}
impl<R: NetworkEligibleStorage> NetworkFinalizationFailure<R> {
    fn storage(failure: R::FinalizationFailure) -> Box<Self> {
        Box::new(Self {
            pending: Pending::Storage(failure),
        })
    }
    fn network(
        failure: Box<NetworkCleanupFailure>,
        completion: TeardownCompletion<R>,
    ) -> Box<Self> {
        Box::new(Self {
            pending: Pending::Network {
                failure,
                completion,
            },
        })
    }
}
impl<R: NetworkEligibleStorage> ExecutionStorage for NetworkedStorage<R> {
    type Finalized = R::Finalized;
    type FinalizationFailure = Box<NetworkFinalizationFailure<R>>;
    fn send(
        &self,
        policy: &LabelPolicy<BoundDisabled>,
        channel: &Supervision<Registered>,
        network: Option<&CommandNetwork>,
        deadline: Instant,
    ) -> io::Result<()> {
        if network.is_some() {
            return Err(io::Error::other("nested command networking is invalid"));
        }
        self.storage
            .send(policy, channel, Some(&self.network), deadline)
    }
    fn finalize(
        completion: TeardownCompletion<Self>,
    ) -> Result<Self::Finalized, Self::FinalizationFailure> {
        let TeardownCompletion { released, deadline } = completion;
        let Released {
            status,
            reaped,
            storage,
        } = released;
        let NetworkedStorage { storage, network } = storage;
        let completion = TeardownCompletion {
            released: Released {
                status,
                reaped,
                storage,
            },
            deadline,
        };
        match network.close(deadline) {
            Ok(()) => R::finalize(completion).map_err(NetworkFinalizationFailure::storage),
            Err(failure) => Err(NetworkFinalizationFailure::network(failure, completion)),
        }
    }
    fn retry_finalization(
        failure: Self::FinalizationFailure,
    ) -> Result<Self::Finalized, Self::FinalizationFailure> {
        match failure.pending {
            Pending::Network {
                failure,
                completion,
            } => match failure.retry() {
                Ok(()) => R::finalize(completion).map_err(NetworkFinalizationFailure::storage),
                Err(failure) => Err(NetworkFinalizationFailure::network(failure, completion)),
            },
            Pending::Storage(failure) => {
                R::retry_finalization(failure).map_err(NetworkFinalizationFailure::storage)
            }
        }
    }
    fn finalization_diagnostics(failure: &Self::FinalizationFailure) -> &FailureDiagnostics {
        match &failure.pending {
            Pending::Network { failure, .. } => failure.diagnostics(),
            Pending::Storage(failure) => R::finalization_diagnostics(failure),
        }
    }
    fn finalized_parts(finalized: Self::Finalized) -> (ReapedChildren, Self::Unmounted) {
        R::finalized_parts(finalized)
    }
}
