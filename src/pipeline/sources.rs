// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::output::StageOutput;
use crate::{
    contract::recipe::StageRef,
    input::ResolvedBuild,
    plan::Copy,
    step::{CommandStorage, SetupBudget},
    storage::{Runtime, Workspace},
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Sources<'a, 'w> {
    pub(super) outputs: &'a BTreeMap<StageRef, StageOutput<'a, 'w>>,
    pub(super) build: &'a ResolvedBuild,
    pub(super) workspace: &'w Workspace,
    pub(super) runtime: &'a Runtime<'w>,
}
impl Sources<'_, '_> {
    pub(crate) fn copy<R: CommandStorage>(
        &self,
        destination: &mut R,
        directive: &Copy,
        budget: &SetupBudget,
    ) -> anyhow::Result<()> {
        let name = directive
            .from()
            .ok_or_else(|| anyhow::anyhow!("named COPY requires a stage"))?;
        let root = self
            .outputs
            .get(name)
            .and_then(|output| output.root.as_ref())
            .ok_or_else(|| anyhow::anyhow!("COPY stage {name} has no persisted root"))?;
        root.read(
            self.build.cache(),
            self.workspace,
            self.runtime,
            |source, _| {
                destination.copy_image(directive, source, budget)?;
                Ok(())
            },
        )
    }
}
