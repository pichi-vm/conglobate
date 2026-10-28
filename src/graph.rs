// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    image::{ImageReference, IndexDigest},
    lock::{ImageLock, LockError},
    plan::{Directive, Plan, Reference, TaskKind},
};
use std::collections::{BTreeMap, BTreeSet};

// Input-use roles describe verification needs, not output ancestry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Stage {
    Bootstrap,
    Carapace,
    Firmware,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum InputRole {
    BootstrapTools,
    CarapaceBase,
    FirmwareTools,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StepId {
    stage: Stage,
    index: usize,
}
impl StepId {
    #[cfg(test)]
    pub(crate) fn stage(self) -> Stage {
        self.stage
    }
    #[cfg(test)]
    pub(crate) fn index(self) -> usize {
        self.index
    }
}
#[derive(Debug)]
pub(crate) struct LockedInput {
    reference: ImageReference,
    index: IndexDigest,
    roles: BTreeSet<InputRole>,
    execution_required: bool,
}
impl LockedInput {
    pub(crate) fn reference(&self) -> &ImageReference {
        &self.reference
    }
    pub(crate) fn index(&self) -> IndexDigest {
        self.index
    }
    #[cfg(test)]
    pub(crate) fn roles(&self) -> &BTreeSet<InputRole> {
        &self.roles
    }
    pub(crate) fn execution_required(&self) -> bool {
        self.execution_required
    }
}
#[derive(Debug)]
pub(crate) struct BuildGraph {
    plan: Plan,
    inputs: Vec<LockedInput>,
    network_steps: Vec<StepId>,
}
impl BuildGraph {
    pub(crate) fn new(plan: Plan, lock: &ImageLock) -> Result<Self, LockError> {
        let mut inputs = BTreeMap::new();
        let mut network_steps = Vec::new();
        // Group network directives by stage while preserving their order across
        // tasks of the same kind. Only the selected task closure is visited.
        let mut offsets = BTreeMap::<Stage, usize>::new();
        for name in plan.order() {
            let task = &plan.tasks()[name];
            let (stage, role) = match task.kind() {
                TaskKind::Bootstrap(_) => (Stage::Bootstrap, InputRole::BootstrapTools),
                TaskKind::Extend => (Stage::Carapace, InputRole::CarapaceBase),
                TaskKind::Build => (Stage::Firmware, InputRole::FirmwareTools),
            };
            if let Reference::Remote(remote) = task.from() {
                let reference: ImageReference = remote.as_ref().parse()?;
                let input = inputs.entry(reference.clone()).or_insert(LockedInput {
                    index: lock.resolve(&reference)?,
                    reference,
                    roles: BTreeSet::new(),
                    execution_required: true,
                });
                input.roles.insert(role);
            }
            let offset = offsets.entry(stage).or_default();
            for (index, directive) in task.directives().iter().enumerate() {
                if matches!(directive, Directive::Run(run) if run.network()) {
                    network_steps.push(StepId {
                        stage,
                        index: *offset + index,
                    });
                }
            }
            *offset += task.directives().len();
        }
        network_steps.sort_by_key(|s| (s.stage, s.index));
        Ok(Self {
            plan,
            inputs: inputs.into_values().collect(),
            network_steps,
        })
    }
    pub(crate) fn execution_required(&self) -> bool {
        self.plan.order().iter().any(|name| {
            let task = &self.plan.tasks()[name];
            !matches!(task.kind(), TaskKind::Extend) || !task.directives().is_empty()
        })
    }
    pub(crate) fn plan(&self) -> &Plan {
        &self.plan
    }
    pub(crate) fn inputs(&self) -> &[LockedInput] {
        &self.inputs
    }
    pub(crate) fn network_steps(&self) -> &[StepId] {
        &self.network_steps
    }
}
