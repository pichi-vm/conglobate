// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Error, Image, ImageRef, Stage, StageRef};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
/// Authored recipe. Call validate before acquiring input or execution resources.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stages: BTreeMap<StageRef, Stage>,
    pub image: Image,
}
impl Plan {
    /// Validate every declaration, including unused stages and their references.
    pub fn validate(&self) -> Result<(), Error> {
        self.image.validate()?;
        for stage in self.stages.values() {
            stage.validate()?;
        }
        let mut done = BTreeSet::new();
        for name in self.stages.keys() {
            self.walk(name, &mut done, &mut Vec::new())?;
        }
        for source in self.image.sources() {
            if let ImageRef::Local(name) = source {
                self.walk(name, &mut done, &mut Vec::new())?;
            }
        }
        Ok(())
    }
    /// Deterministic topological order of only the selected dependency closure.
    pub fn order(&self) -> Result<Vec<StageRef>, Error> {
        self.validate()?;
        let mut order = Vec::new();
        let mut done = BTreeSet::new();
        for source in self.image.sources() {
            if let ImageRef::Local(name) = source {
                self.walk(name, &mut done, &mut order)?;
            }
        }
        Ok(order)
    }
    fn walk(
        &self,
        start: &StageRef,
        done: &mut BTreeSet<StageRef>,
        order: &mut Vec<StageRef>,
    ) -> Result<(), Error> {
        let mut visiting = BTreeSet::new();
        let mut stack = vec![(start.clone(), false)];
        while let Some((name, exit)) = stack.pop() {
            if exit {
                visiting.remove(&name);
                done.insert(name.clone());
                order.push(name);
                continue;
            }
            if done.contains(&name) {
                continue;
            }
            let stage = self
                .stages
                .get(&name)
                .ok_or_else(|| Error(format!("unknown stage {name}")))?;
            if !visiting.insert(name.clone()) {
                return Err(Error(format!("stage dependency cycle at {name}")));
            }
            stack.push((name, true));
            for dependency in stage.dependencies().into_iter().rev() {
                stack.push((dependency, false));
            }
        }
        Ok(())
    }
}
