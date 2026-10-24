// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Protected command lifetime. Trusted helper construction and sandbox readiness
//! remain prerequisites; this owner couples command admission to its own drain.
use crate::{
    domain::ContainedHelper,
    isolation::{Mapped, UserNamespace},
    process::{ChildReaper, Joined, ReapedChildren, Supervision},
};
use std::{io, marker::PhantomData, mem::ManuallyDrop, process::ExitStatus, time::Instant};

pub(crate) mod execution;
mod preparation;
pub(crate) use preparation::{CommandSandbox, SandboxDirectories};
mod finalize;
pub(crate) mod outcome;
pub(crate) use outcome::CommandOutcome;
mod cutoff;
mod setup;
pub(crate) use cutoff::SetupCutoff;
pub(crate) use setup::{CommandStartFailure, RunningCommand, SetupBudget};
mod teardown;
pub(crate) use teardown::TeardownCompletion;

mod private {
    pub(crate) trait Sealed {}
    pub(crate) trait Storage {}
}
/// Storage held through command cancellation, reaping and namespace release.
/// Implementations are sealed so command lifetimes cannot be replaced with an
/// arbitrary borrowed value. More stage-specific volume sets can be added here.
pub(crate) trait StepStorage: private::Storage + std::fmt::Debug {}
impl private::Storage for () {}
impl StepStorage for () {}
impl private::Storage for crate::storage::Volume<'_, '_, '_, crate::storage::Mounted> {}
impl StepStorage for crate::storage::Volume<'_, '_, '_, crate::storage::Mounted> {}

/// Unfinished steps intentionally retain storage through dedicated-VM disposal.
/// Drop cannot prove descendant quiescence, even if child termination succeeds.
#[derive(Debug)]
struct Retained<R: StepStorage>(ManuallyDrop<R>);
impl<R: StepStorage> Retained<R> {
    fn new(storage: R) -> Self {
        Self(ManuallyDrop::new(storage))
    }
    fn release(self) -> R {
        ManuallyDrop::into_inner(self.0)
    }
}

pub(crate) trait StepState<'lease>: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
    fn cleanup(
        resources: Self::Resources,
        deadline: Instant,
    ) -> io::Result<(ExitStatus, ReapedChildren)>;
}
#[derive(Debug)]
pub(crate) struct Ready;
#[derive(Debug)]
pub(crate) struct Running;
impl private::Sealed for Ready {}
impl private::Sealed for Running {}
impl<'a> StepState<'a> for Ready {
    type Resources = (ContainedHelper, ChildReaper);
    fn cleanup(
        (helper, reaper): Self::Resources,
        deadline: Instant,
    ) -> io::Result<(ExitStatus, ReapedChildren)> {
        helper.finish_until(reaper, deadline)
    }
}
impl<'a> StepState<'a> for Running {
    type Resources = (ContainedHelper, ChildReaper);
    fn cleanup(
        (helper, reaper): Self::Resources,
        deadline: Instant,
    ) -> io::Result<(ExitStatus, ReapedChildren)> {
        helper.finish_until(reaper, deadline)
    }
}
/// Owns assigned storage until the contained helper and every descendant stop.
/// Preparation/admission have distinct states. Cleanup is a private linear
/// sequence, not an externally recoverable state machine.
#[derive(Debug)]
pub(crate) struct Step<'lease, S: StepState<'lease>, R: StepStorage = ()> {
    storage: Retained<R>,
    namespace: ManuallyDrop<UserNamespace<Mapped>>,
    network: ManuallyDrop<Option<crate::network::CommandNetwork>>,
    resources: S::Resources,
    _lease: PhantomData<&'lease ()>,
}
#[derive(Debug, thiserror::Error)]
#[error("protected step transition: {source}")]
pub(crate) struct StepFailure<'lease, S: StepState<'lease>, R: StepStorage = ()> {
    source: io::Error,
    step: Step<'lease, S, R>,
}
impl<'lease, S: StepState<'lease>, R: StepStorage> StepFailure<'lease, S, R> {
    pub(crate) fn into_parts(self) -> (io::Error, Step<'lease, S, R>) {
        (self.source, self.step)
    }
}
impl<'lease> Step<'lease, Ready> {
    fn with_storage<R: StepStorage>(self, storage: R) -> Step<'lease, Ready, R> {
        Step {
            storage: Retained::new(storage),
            namespace: self.namespace,
            network: self.network,
            resources: self.resources,
            _lease: PhantomData,
        }
    }

    /// The helper must be trusted, contained and blocked on admission. The step
    /// owns its mapped namespace and assigned storage. Install the reaper
    /// before spawning it. No other process may admit commands to this scope.
    pub(crate) fn new(
        namespace: UserNamespace<Mapped>,
        helper: ContainedHelper,
        reaper: ChildReaper,
    ) -> Self {
        Self {
            storage: Retained::new(()),
            namespace: ManuallyDrop::new(namespace),
            network: ManuallyDrop::new(None),
            resources: (helper, reaper),
            _lease: PhantomData,
        }
    }
}
impl<'lease, R: StepStorage> Step<'lease, Ready, R> {
    fn running(self) -> Step<'lease, Running, R> {
        Step {
            storage: self.storage,
            namespace: self.namespace,
            network: self.network,
            resources: self.resources,
            _lease: PhantomData,
        }
    }
}

impl<R: StepStorage> Step<'_, Running, R> {
    pub(crate) fn supervision(&self) -> &Supervision<Joined> {
        self.resources.0.supervision()
    }
}
impl Step<'_, Ready, crate::storage::Volume<'_, '_, '_, crate::storage::Mounted>> {
    /// Send only the owned volume, using this step’s owned mapped namespace.
    /// Failure leaves the entire step owned and available for cancellation.
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn send_volume_until(&self, deadline: Instant) -> io::Result<()> {
        let volume = &self.storage.0;
        let namespace = &self.namespace;
        self.resources
            .0
            .supervision()
            .send_volume_until(&volume.view()?, namespace, deadline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};
    #[derive(Debug)]
    struct Storage(Rc<Cell<usize>>);
    impl private::Storage for Storage {}
    impl StepStorage for Storage {}
    impl Drop for Storage {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    #[test]
    fn unfinished_storage_is_retained_but_explicit_release_returns_ownership() {
        let drops = Rc::new(Cell::new(0));
        {
            let _retained = Retained::new(Storage(Rc::clone(&drops)));
        }
        assert_eq!(drops.get(), 0);
        let returned = Retained::new(Storage(Rc::clone(&drops))).release();
        assert_eq!(drops.get(), 0);
        drop(returned);
        assert_eq!(drops.get(), 1);
    }
}

mod bootstrap;

mod directives;

impl<R: StepStorage> Step<'_, Ready, R> {
    pub(crate) fn with_network(mut self, network: crate::network::CommandNetwork) -> Self {
        assert!(self.network.is_none(), "command network already assigned");
        *self.network = Some(network);
        self
    }
}

mod storage;
pub(crate) use storage::CommandStorage;
