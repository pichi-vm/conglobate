// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    contract::recipe::{ImageRef, StageRef},
    image::{ImageReference, IndexDigest},
    lock::{ImageLock, LockError},
    plan::{Directive, Plan},
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct LockedInput {
    reference: ImageReference,
    index: IndexDigest,
    root_required: bool,
}
impl LockedInput {
    pub(crate) fn reference(&self) -> &ImageReference {
        &self.reference
    }
    pub(crate) fn index(&self) -> IndexDigest {
        self.index
    }
    pub(crate) fn root_required(&self) -> bool {
        self.root_required
    }
}
#[derive(Debug)]
pub(crate) struct BuildGraph {
    plan: Plan,
    inputs: Vec<LockedInput>,
    network_steps: Vec<(StageRef, usize)>,
}
impl BuildGraph {
    pub(crate) fn new(plan: Plan, lock: &ImageLock) -> Result<Self, LockError> {
        let mut inputs = BTreeMap::new();
        for remote in plan.remote_sources() {
            let reference: ImageReference = remote.as_ref().parse()?;
            inputs.entry(reference.clone()).or_insert(LockedInput {
                index: lock.resolve(&reference)?,
                reference,
                root_required: false,
            });
        }
        let mut network_steps = Vec::new();
        for name in &plan.order {
            let stage = &plan.stages[name];
            if let ImageRef::Remote(remote) = &stage.from {
                inputs
                    .get_mut(&remote.as_ref().parse::<ImageReference>()?)
                    .expect("selected source")
                    .root_required = true;
            }
            for (index, directive) in stage.derive.iter().enumerate() {
                if matches!(directive, Directive::Exec(exec) if exec.network) {
                    network_steps.push((name.clone(), index));
                }
            }
        }
        Ok(Self {
            plan,
            inputs: inputs.into_values().collect(),
            network_steps,
        })
    }
    pub(crate) fn execution_required(&self) -> bool {
        self.plan.commands_required() || self.plan.root_reader_required()
    }
    pub(crate) fn plan(&self) -> &Plan {
        &self.plan
    }
    pub(crate) fn inputs(&self) -> &[LockedInput] {
        &self.inputs
    }
    pub(crate) fn network_steps(&self) -> &[(StageRef, usize)] {
        &self.network_steps
    }
}
