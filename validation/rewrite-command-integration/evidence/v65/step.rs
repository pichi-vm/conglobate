// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Protected command lifetime. Trusted helper construction and sandbox readiness
//! remain prerequisites; this owner couples policy admission to its own drain.
use crate::{
    domain::{ContainedHelper, DescendantDrain, DrainedHelper, HelperCancellation, HelperReaping},
    policy::{BoundDisabled, LabelPolicy, PolicyError},
    process::{Admitted, ChildReaper, ReapedChildren, Supervision},
};
use libbpf_rs::{MapCore, MapFlags, MapHandle};
use std::{
    io, marker::PhantomData, mem::ManuallyDrop, path::Path, process::ExitStatus, time::Instant,
};

mod private {
    pub trait Sealed {}
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

/// No policy or identity handle escapes before this command's quiescence.
/// Dropping an unfinished step retains pinned authorization through VM disposal;
/// best-effort child destruction is never used as a quiescence receipt.
///
/// ```compile_fail
/// use conglobate::step::{Step, Running};
/// fn cannot_disable_while_running(step: Step<'_, Running>) {
///     step.disable();
/// }
/// ```
/// ```compile_fail
/// use conglobate::step::{Step, Reaping};
/// fn cannot_skip_descendant_drain(step: Step<'_, Reaping>) {
///     step.finish_record(std::path::Path::new("/unused"));
/// }
/// ```
#[derive(Debug)]
pub struct Step<'lease, S: StepState<'lease>> {
    policy: PolicyGuard,
    resources: S::Resources,
    _lease: PhantomData<&'lease ()>,
}
#[derive(Debug)]
struct PolicyGuard {
    policy: ManuallyDrop<LabelPolicy<BoundDisabled>>,
    config: MapHandle,
}
impl PolicyGuard {
    fn activate(&self) -> Result<(), PolicyError> {
        self.config.update(
            &0u32.to_ne_bytes(),
            &self.policy.scope().subject_value(),
            MapFlags::ANY,
        )?;
        Ok(())
    }
    fn disable(self) -> Result<LabelPolicy<BoundDisabled>, (PolicyError, Self)> {
        if let Err(source) = self
            .config
            .update(&0u32.to_ne_bytes(), &[0; 8], MapFlags::ANY)
        {
            return Err((source.into(), self));
        }
        Ok(ManuallyDrop::into_inner(self.policy))
    }
}
#[derive(Debug, thiserror::Error)]
#[error("protected step preparation: {source}")]
pub struct PrepareFailure<'lease> {
    source: PolicyError,
    policy: LabelPolicy<BoundDisabled>,
    helper: ContainedHelper<'lease>,
    reaper: ChildReaper,
}
impl<'lease> PrepareFailure<'lease> {
    pub fn into_parts(
        self,
    ) -> (
        PolicyError,
        LabelPolicy<BoundDisabled>,
        ContainedHelper<'lease>,
        ChildReaper,
    ) {
        (self.source, self.policy, self.helper, self.reaper)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("protected step transition: {source}")]
pub struct StepFailure<'lease, S: StepState<'lease>> {
    source: io::Error,
    step: Step<'lease, S>,
}
impl<'lease, S: StepState<'lease>> StepFailure<'lease, S> {
    pub fn into_parts(self) -> (io::Error, Step<'lease, S>) {
        (self.source, self.step)
    }
}
#[derive(Debug, thiserror::Error)]
pub enum StartFailure<'lease> {
    #[error("policy activation: {0}")]
    Activation(Box<StepFailure<'lease, Ready>>),
    #[error("command admission: {0}")]
    Admission(Box<StepFailure<'lease, Cancelling>>),
}
impl<'lease> Step<'lease, Ready> {
    /// The helper must be trusted, registered and blocked on admission; its
    /// namespace/filesystems must match the bound policy. Install the reaper
    /// before spawning it. No other process may admit commands to this scope.
    pub fn new(
        policy: LabelPolicy<BoundDisabled>,
        helper: ContainedHelper<'lease>,
        reaper: ChildReaper,
    ) -> Result<Self, Box<PrepareFailure<'lease>>> {
        let config = match policy.config_handle() {
            Ok(config) => config,
            Err(source) => {
                return Err(Box::new(PrepareFailure {
                    source,
                    policy,
                    helper,
                    reaper,
                }));
            }
        };
        Ok(Self {
            policy: PolicyGuard {
                policy: ManuallyDrop::new(policy),
                config,
            },
            resources: (helper, reaper),
            _lease: PhantomData,
        })
    }
    pub fn admit_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Running>, StartFailure<'lease>> {
        if let Err(source) = self.policy.activate() {
            return Err(StartFailure::Activation(Box::new(StepFailure {
                source: io::Error::other(source),
                step: self,
            })));
        }
        let (helper, reaper) = self.resources;
        match helper.admit_until(deadline) {
            Ok(helper) => Ok(Step {
                policy: self.policy,
                resources: (helper, reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(StartFailure::Admission(Box::new(StepFailure {
                    source,
                    step: Step {
                        policy: self.policy,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                })))
            }
        }
    }
}

impl<'lease> Step<'lease, Ready> {
    pub fn cancel_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Reaping>, Box<StepFailure<'lease, Cancelling>>> {
        let (helper, reaper) = self.resources;
        match helper.cancel_until(deadline) {
            Ok(helper) => Ok(Step {
                policy: self.policy,
                resources: (helper, reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        policy: self.policy,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}

impl<'lease> Step<'lease, Running> {
    pub fn cancel_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Reaping>, Box<StepFailure<'lease, Cancelling>>> {
        let (helper, reaper) = self.resources;
        match helper.cancel_until(deadline) {
            Ok(helper) => Ok(Step {
                policy: self.policy,
                resources: (helper, reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        policy: self.policy,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}

impl<'lease> Step<'lease, Cancelling> {
    pub fn cancel_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Reaping>, Box<StepFailure<'lease, Cancelling>>> {
        let (helper, reaper) = self.resources;
        match helper.retry_until(deadline) {
            Ok(helper) => Ok(Step {
                policy: self.policy,
                resources: (helper, reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        policy: self.policy,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}
impl Step<'_, Running> {
    pub fn supervision(&self) -> &Supervision<Admitted> {
        self.resources.0.supervision()
    }
}
impl<'lease> Step<'lease, Reaping> {
    pub fn reap_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Draining>, Box<StepFailure<'lease, Reaping>>> {
        let (helper, reaper) = self.resources;
        match helper.reap_until(deadline) {
            Ok(helper) => Ok(Step {
                policy: self.policy,
                resources: helper.with_reaper(reaper),
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source: io::Error::other(source),
                    step: Step {
                        policy: self.policy,
                        resources: (helper, reaper),
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}
impl<'lease> Step<'lease, Draining> {
    pub fn drain_until(
        self,
        deadline: Instant,
    ) -> Result<Step<'lease, Quiescent>, Box<StepFailure<'lease, Draining>>> {
        match self.resources.drain_until(deadline) {
            Ok(resources) => Ok(Step {
                policy: self.policy,
                resources,
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, resources) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        policy: self.policy,
                        resources,
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}
impl<'lease> Step<'lease, Quiescent> {
    pub fn finish_record(
        self,
        parent: &Path,
    ) -> Result<Step<'lease, Finishing>, Box<StepFailure<'lease, Quiescent>>> {
        match self.resources.finish(parent) {
            Ok(resources) => Ok(Step {
                policy: self.policy,
                resources,
                _lease: PhantomData,
            }),
            Err(failure) => {
                let (source, resources) = failure.into_parts();
                Err(Box::new(StepFailure {
                    source,
                    step: Step {
                        policy: self.policy,
                        resources,
                        _lease: PhantomData,
                    },
                }))
            }
        }
    }
}
impl<'lease> Step<'lease, Finishing> {
    pub fn disable(
        self,
    ) -> Result<
        (ExitStatus, ReapedChildren, LabelPolicy<BoundDisabled>),
        Box<StepFailure<'lease, Finishing>>,
    > {
        match self.policy.disable() {
            Ok(policy) => Ok((self.resources.0, self.resources.1, policy)),
            Err((source, policy)) => Err(Box::new(StepFailure {
                source: io::Error::other(source),
                step: Step {
                    policy,
                    resources: self.resources,
                    _lease: PhantomData,
                },
            })),
        }
    }
}
