// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CommandNetwork, ResolverInput, ResolverSource};
use crate::{step::SetupBudget, storage::Workspace};
use std::io;

/// Captured build environment used only for explicitly enabled RUN directives.
#[derive(Debug)]
pub(crate) struct CommandNetworking<'a> {
    input: &'a ResolverInput,
    workspace: &'a Workspace,
}
impl<'a> CommandNetworking<'a> {
    pub(crate) fn new(input: &'a ResolverInput, workspace: &'a Workspace) -> Self {
        Self { input, workspace }
    }
    pub(crate) fn prepare(&self, budget: &SetupBudget) -> io::Result<CommandNetwork> {
        budget.check()?;
        let resolver = ResolverSource::prepare(self.input, self.workspace)?;
        // The step owns this source before its next setup-budget check.
        Ok(CommandNetwork::new(resolver))
    }
}
