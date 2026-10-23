// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Protected command lifetime. Trusted helper construction and sandbox readiness
//! remain prerequisites; this owner couples command admission to its own drain.
use crate::{
    domain::{ContainedHelper, DescendantDrain, DrainedHelper, HelperCancellation, HelperReaping},
    isolation::{Mapped, UserNamespace},
    process::{Admitted, ChildReaper, ReapedChildren, Supervision},
};
use std::{
    io, marker::PhantomData, mem::ManuallyDrop, path::Path, process::ExitStatus, time::Instant,
};

mod execution;
pub use execution::{
    ExecutedBootstrap, ExecutedStorage, ExecutedVolume, ExecutionFailure, ExecutionOutcome,
    ExecutionStorage, StorageExecutionFailure,
};
mod preparation;
pub use preparation::{
    BootstrapPreparation, BootstrapPreparationFailure, CommandPreparation, CommandSandbox,
    PreparationFailure, PreparationStorage, SandboxDirectories, StorageLayout, StoragePreparation,
    StoragePreparationFailure, StorageRollback,
};
mod finalize;
mod outcome;
pub use outcome::{CommandOutcome, CommandResult, ObservedCommand};
mod cutoff;
mod setup;
pub use cutoff::SetupCutoff;
pub use setup::{CommandStartFailure, PreparedStep, RunningCommand, SetupBudget};
mod teardown;
pub use finalize::{
    BootstrapFinalizationFailure, FinalizationFailure, FinalizedBootstrap, FinalizedStorage,
    FinalizedVolume, StorageFinalizationFailure,
};
pub use teardown::{TeardownCompletion, TeardownFailure};

mod private {
    pub trait Sealed {}
    pub trait Storage {}
}
/// Storage held through command cancellation, reaping and namespace release.
/// Implementations are sealed so command lifetimes cannot be replaced with an
/// arbitrary borrowed value. More stage-specific volume sets can be added here.
pub trait StepStorage: private::Storage + std::fmt::Debug {}
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

pub trait StepState<'lease>: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub struct Ready;
#[derive(Debug)]
pub struct Running;
#[derive(Debug)]
pub struct Cancelling;
#[derive(Debug)]
pub struct Reaping;
#[derive(Debug)]
pub struct Draining;
#[derive(Debug)]
pub struct Quiescent;
#[derive(Debug)]
pub struct Finishing;
impl private::Sealed for Ready {}
impl private::Sealed for Running {}
impl private::Sealed for Cancelling {}
impl private::Sealed for Reaping {}
impl private::Sealed for Draining {}
impl private::Sealed for Quiescent {}
impl private::Sealed for Finishing {}
impl<'a> StepState<'a> for Ready {
    type Resources = (ContainedHelper<'a>, ChildReaper);
}
impl<'a> StepState<'a> for Running {
    type Resources = (ContainedHelper<'a, Admitted>, ChildReaper);
}
impl<'a> StepState<'a> for Cancelling {
    type Resources = (HelperCancellation<'a>, ChildReaper);
}
impl<'a> StepState<'a> for Reaping {
    type Resources = (HelperReaping<'a>, ChildReaper);
}
impl<'a> StepState<'a> for Draining {
    type Resources = DescendantDrain<'a>;
}
impl<'a> StepState<'a> for Quiescent {
    type Resources = DrainedHelper<'a>;
}
impl<'a> StepState<'a> for Finishing {
    type Resources = (ExitStatus, ReapedChildren);
}

/// No namespace or storage handle escapes before this command's quiescence.
/// Dropping an unfinished step retains namespace and storage through VM disposal;
/// best-effort child destruction is never used as a quiescence receipt.
///
/// Admission consumes Ready through `admit_until`, producing Running on success.
/// Failed admission retains Cancelling.
/// Cleanup follows these consuming transitions:
///
/// ```text
/// Ready | Running | Cancelling
///     --cancel_until--> Reaping --reap_until--> Draining
///     --drain_until--> Quiescent --finish_record--> Finishing
///     --finish--> StepCompletion
/// ```
///
/// Failed cancellation retains Cancelling; subsequent transition failures
/// retain their input state. `teardown` drives this sequence and releases the namespace descriptor under one fixed deadline, including retries.
/// Its TeardownCompletion still owns storage: explicit unmount/flush and sealing
/// remain necessary before publication. Normal command exit uses this same
/// cleanup sequence; it does not skip descendant cancellation or drain.
///
/// ```compile_fail
/// use conglobate::step::{Step, Running};
/// fn cannot_finish_while_running(step: Step<'_, Running>) {
///     step.finish();
/// }
/// ```
/// ```compile_fail
/// use conglobate::step::{Step, Reaping};
/// fn cannot_skip_descendant_drain(step: Step<'_, Reaping>) {
///     step.finish_record(std::path::Path::new("/unused"));
/// }
/// ```
/// ```compile_fail
/// use conglobate::step::{Step, Cancelling};
/// fn cannot_reopen_admission(step: Step<'_, Cancelling>) {
///     step.admit_until(std::time::Instant::now());
/// }
/// ```
#[derive(Debug)]
pub struct Step<'lease, S: StepState<'lease>, R: StepStorage = ()> {
    storage: Retained<R>,
    namespace: ManuallyDrop<UserNamespace<Mapped>>,
    resources: S::Resources,
    _lease: PhantomData<&'lease ()>,
}
#[derive(Debug, thiserror::Error)]
#[error("protected step transition: {source}")]
pub struct StepFailure<'lease, S: StepState<'lease>, R: StepStorage = ()> {
    source: io::Error,
    step: Step<'lease, S, R>,
}
impl<'lease, S: StepState<'lease>, R: StepStorage> StepFailure<'lease, S, R> {
    pub fn into_parts(self) -> (io::Error, Step<'lease, S, R>) {
        (self.source, self.step)
    }
}
pub type StartFailure<'lease, R = ()> = Box<StepFailure<'lease, Cancelling, R>>;
impl<'lease> Step<'lease, Ready> {
    fn with_storage<R: StepStorage>(self, storage: R) -> Step<'lease, Ready, R> {
        Step {
            storage: Retained::new(storage),
            namespace: self.namespace,
            resources: self.resources,
            _lease: PhantomData,
        }
    }

    /// The helper must be trusted, contained and blocked on admission. The step
    /// owns its mapped namespace and assigned storage. Install the reaper
    /// before spawning it. No other process may admit commands to this scope.
    pub fn new(
        namespace: UserNamespace<Mapped>,
        helper: ContainedHelper<'lease>,
        reaper: ChildReaper,
    ) -> Self {
        Self {
            storage: Retained::new(()),
            namespace: ManuallyDrop::new(namespace),
            resources: (helper, reaper),
            _lease: PhantomData,
        }
    }
    /// ```compile_fail
    /// use conglobate::{step::{Step, Ready}, storage::{Volume, Mounted}};
    /// fn cannot_unmount_after_assignment(step: Step<'_, Ready>, volume: Volume<'_, '_, '_, Mounted>) {
    ///     let step = step.with_volume(volume);
    ///     volume.unmount();
    ///     drop(step);
    /// }
    /// ```
    /// ```compile_fail
    /// use conglobate::{step::{Step, Running}, storage::{Volume, Mounted}};
    /// fn cannot_release_running_volume(step: Step<'_, Running, Volume<'_, '_, '_, Mounted>>) {
    ///     step.finish();
    /// }
    /// ```
    /// Consume the mounted owner before admission. No storage accessor exists
    /// on active or cleanup states; only successful finish returns it.
    pub fn with_volume<'ns, 'origin, 'workspace>(
        self,
        volume: crate::storage::Volume<'ns, 'origin, 'workspace, crate::storage::Mounted>,
    ) -> Step<
        'lease,
        Ready,
        crate::storage::Volume<'ns, 'origin, 'workspace, crate::storage::Mounted>,
    > {
        Step {
            storage: Retained::new(volume),
            namespace: self.namespace,
            resources: self.resources,
            _lease: PhantomData,
        }
    }
}
impl<'lease, R: StepStorage> Step<'lease, Ready, R> {
    pub fn admit_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Running, R>, StartFailure<'lease, R>> {
        let (helper, reaper) = self.resources;
        match helper.admit_until(deadline) {
            Ok(helper) => Ok(Step {
                storage: self.storage,
                namespace: self.namespace,
                resources: (helper, reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        storage: self.storage,
                        namespace: self.namespace,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}

impl<'lease, R: StepStorage> Step<'lease, Ready, R> {
    pub fn cancel_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Reaping, R>, Box<StepFailure<'lease, Cancelling, R>>> {
        let (helper, reaper) = self.resources;
        match helper.cancel_until(deadline) {
            Ok(helper) => Ok(Step {
                storage: self.storage,
                namespace: self.namespace,
                resources: (helper, reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        storage: self.storage,
                        namespace: self.namespace,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}

impl<'lease, R: StepStorage> Step<'lease, Running, R> {
    pub fn cancel_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Reaping, R>, Box<StepFailure<'lease, Cancelling, R>>> {
        let (helper, reaper) = self.resources;
        match helper.cancel_until(deadline) {
            Ok(helper) => Ok(Step {
                storage: self.storage,
                namespace: self.namespace,
                resources: (helper, reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        storage: self.storage,
                        namespace: self.namespace,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}

impl<'lease, R: StepStorage> Step<'lease, Cancelling, R> {
    pub fn cancel_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Reaping, R>, Box<StepFailure<'lease, Cancelling, R>>> {
        let (helper, reaper) = self.resources;
        match helper.retry_until(deadline) {
            Ok(helper) => Ok(Step {
                storage: self.storage,
                namespace: self.namespace,
                resources: (helper, reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        storage: self.storage,
                        namespace: self.namespace,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}
impl<R: StepStorage> Step<'_, Running, R> {
    pub fn supervision(&self) -> &Supervision<Admitted> {
        self.resources.0.supervision()
    }
}
impl<'lease, R: StepStorage> Step<'lease, Reaping, R> {
    pub fn reap_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Draining, R>, Box<StepFailure<'lease, Reaping, R>>> {
        let (helper, reaper) = self.resources;
        match helper.reap_until(deadline) {
            Ok(helper) => Ok(Step {
                storage: self.storage,
                namespace: self.namespace,
                resources: helper.with_reaper(reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source: io::Error::other(source),
                    step: Step {
                        storage: self.storage,
                        namespace: self.namespace,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}
impl<'lease, R: StepStorage> Step<'lease, Draining, R> {
    pub fn drain_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Quiescent, R>, Box<StepFailure<'lease, Draining, R>>> {
        match self.resources.drain_until(deadline) {
            Ok(resources) => Ok(Step {
                storage: self.storage,
                namespace: self.namespace,
                resources,
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, resources) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        storage: self.storage,
                        namespace: self.namespace,
                        resources,
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}
impl<'lease, R: StepStorage> Step<'lease, Quiescent, R> {
    pub fn finish_record(
        self,
        parent: &Path,
    ) -> Result<Step<'lease, Finishing, R>, Box<StepFailure<'lease, Quiescent, R>>> {
        match self.resources.finish(parent) {
            Ok(resources) => Ok(Step {
                storage: self.storage,
                namespace: self.namespace,
                resources,
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, resources) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        storage: self.storage,
                        namespace: self.namespace,
                        resources,
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}
/// Command descendants are quiescent. Storage still requires clean finalization.
#[derive(Debug)]
pub struct StepCompletion<R: StepStorage> {
    status: ExitStatus,
    reaped: ReapedChildren,
    namespace: UserNamespace<Mapped>,
    storage: R,
}
impl<R: StepStorage> StepCompletion<R> {
    pub fn into_parts(self) -> (ExitStatus, ReapedChildren, UserNamespace<Mapped>, R) {
        (self.status, self.reaped, self.namespace, self.storage)
    }
}
impl<R: StepStorage> Step<'_, Finishing, R> {
    pub fn finish(self) -> StepCompletion<R> {
        StepCompletion {
            status: self.resources.0,
            reaped: self.resources.1,
            namespace: ManuallyDrop::into_inner(self.namespace),
            storage: self.storage.release(),
        }
    }
}

impl Step<'_, Ready, crate::storage::Volume<'_, '_, '_, crate::storage::Mounted>> {
    /// Send only the owned volume, using this step’s owned mapped namespace.
    /// Failure leaves the entire step owned and available for cancellation.
    pub fn send_volume_until(&self, deadline: Instant) -> io::Result<()> {
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
pub use directives::{
    CompletedDirectives, CopyStageFailure, DirectiveStorage, NetworkStageFailure, StageFailure,
};

mod network;
pub use network::{NetworkEligibleStorage, NetworkFinalizationFailure, NetworkedStorage};
