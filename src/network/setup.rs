// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CommandNetwork, NetworkNamespace, ResolverInput, ResolverSource};
use crate::{step::SetupBudget, storage::Workspace};
use std::io;

/// Captured environment inputs. No namespace path is reopened during execution.
#[derive(Debug)]
pub(crate) struct NetworkInput {
    namespace: NetworkNamespace,
    resolver: ResolverInput,
}
impl NetworkInput {
    pub(crate) fn new(namespace: NetworkNamespace, resolver: ResolverInput) -> Self {
        Self {
            namespace,
            resolver,
        }
    }
}

/// Captured build environment used only for explicitly enabled RUN directives.
#[derive(Debug)]
pub(crate) struct CommandNetworking<'a> {
    input: &'a NetworkInput,
    workspace: &'a Workspace,
}
impl<'a> CommandNetworking<'a> {
    pub(crate) fn new(input: &'a NetworkInput, workspace: &'a Workspace) -> Self {
        Self { input, workspace }
    }
    pub(crate) fn prepare(&self, budget: &SetupBudget) -> io::Result<CommandNetwork> {
        budget.check()?;
        let namespace = self.input.namespace.try_clone()?;
        let resolver = ResolverSource::prepare(&self.input.resolver, self.workspace)?;
        // The step owns this source before its next setup-budget check.
        Ok(CommandNetwork::new(namespace, resolver))
    }
}
