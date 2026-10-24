// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Immutable stage order and image-use requirements; no execution or fetching.
use crate::{
    image::{ImageReference, IndexDigest},
    lock::{ImageLock, LockError},
    recipe::{Directive, Recipe},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Stage {
    Bootstrap,
    Carapace,
    Firmware,
    Transform,
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
    pub(crate) fn stage(self) -> Stage {
        self.stage
    }
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
    pub(crate) fn roles(&self) -> &BTreeSet<InputRole> {
        &self.roles
    }
    /// Requires a filesystem-bearing image, not merely an image with firmware.
    pub(crate) fn execution_required(&self) -> bool {
        self.execution_required
    }
}
#[derive(Debug)]
pub(crate) struct BuildPlan {
    recipe: Recipe,
    stages: Vec<Stage>,
    inputs: Vec<LockedInput>,
    network_steps: Vec<StepId>,
}
impl BuildPlan {
    pub(crate) fn new(recipe: Recipe, lock: &ImageLock) -> Result<Self, LockError> {
        let mut inputs = BTreeMap::<&ImageReference, LockedInput>::new();
        let mut stages = Vec::new();
        let mut network_steps = Vec::new();
        let mut declarations = Vec::new();
        if let Some(bootstrap) = recipe.bootstrap() {
            stages.push(Stage::Bootstrap);
            declarations.push((Stage::Bootstrap, bootstrap.directives()));
            inputs.insert(
                bootstrap.from(),
                LockedInput {
                    reference: bootstrap.from().clone(),
                    index: lock.resolve(bootstrap.from())?,
                    roles: BTreeSet::from([InputRole::BootstrapTools]),
                    execution_required: true,
                },
            );
        }
        if let Some(carapace) = recipe.carapace() {
            stages.push(Stage::Carapace);
            declarations.push((Stage::Carapace, carapace.directives()));
            if let Some(reference) = carapace.from() {
                let execution_required = !carapace.directives().is_empty()
                    || recipe.firmware().is_some_and(|f| f.from().is_none());
                let input = inputs.entry(reference).or_insert(LockedInput {
                    reference: reference.clone(),
                    index: lock.resolve(reference)?,
                    roles: BTreeSet::new(),
                    execution_required: false,
                });
                input.roles.insert(InputRole::CarapaceBase);
                input.execution_required |= execution_required;
            }
        }
        if let Some(firmware) = recipe.firmware() {
            stages.push(Stage::Firmware);
            declarations.push((Stage::Firmware, firmware.directives()));
            if let Some(reference) = firmware.from() {
                let input = inputs.entry(reference).or_insert(LockedInput {
                    reference: reference.clone(),
                    index: lock.resolve(reference)?,
                    roles: BTreeSet::new(),
                    execution_required: false,
                });
                input.roles.insert(InputRole::FirmwareTools);
                input.execution_required = true;
            }
        }
        for (stage, directives) in declarations {
            for (index, directive) in directives.iter().enumerate() {
                if matches!(directive,Directive::Run(run) if run.network()) {
                    network_steps.push(StepId { stage, index });
                }
            }
        }
        // The implicit transform still executes its applicability check.
        stages.push(Stage::Transform);
        let inputs = inputs.into_values().collect();
        Ok(Self {
            recipe,
            stages,
            inputs,
            network_steps,
        })
    }
    pub(crate) fn execution_required(&self) -> bool {
        self.recipe.bootstrap().is_some()
            || self.recipe.firmware().is_some()
            || self
                .recipe
                .carapace()
                .is_some_and(|stage| !stage.directives().is_empty())
    }
    pub(crate) fn recipe(&self) -> &Recipe {
        &self.recipe
    }
    pub(crate) fn stages(&self) -> &[Stage] {
        &self.stages
    }
    pub(crate) fn inputs(&self) -> &[LockedInput] {
        &self.inputs
    }
    pub(crate) fn network_steps(&self) -> &[StepId] {
        &self.network_steps
    }
}
