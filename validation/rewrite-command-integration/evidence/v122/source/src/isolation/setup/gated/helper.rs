// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{GatedCommandSetup, GatedRootSetup};
use crate::{
    isolation::{
        CommandRoot, ExecutionEndpoint, NamespaceFailure, PrivateMountNamespace, RestrictedCommand,
    },
    process::{CommandPrivileges, CommandSyscalls, WaitFailure},
};
use std::{
    convert::Infallible,
    io,
    process::{Command, ExitStatus},
    time::Instant,
};

#[derive(Debug, thiserror::Error)]
pub enum HelperExecutionError {
    #[error("namespace helper setup: {0}")]
    Setup(#[from] io::Error),
    #[error("namespace child wait: {0}")]
    Wait(#[from] WaitFailure),
}
impl GatedCommandSetup {
    /// Dedicated trusted helper only, already inside the supervisor-owned
    /// containment domain. Irreversibly enter namespaces, launch trusted root
    /// entry as PID 1, then wait. The supervisor enforces command timeout by
    /// cancelling the domain; this helper imposes no shorter execution limit.
    /// The caller must exit on return, including on setup failure.
    pub fn execute(self, mut root_entry: Command) -> Result<ExitStatus, HelperExecutionError> {
        let (scope, root) = self.enter()?;
        root_entry.env_clear().stdin(root.into_input()?);
        let child = scope.spawn(root_entry)?;
        Ok(child.wait_under_supervisor()?.status())
    }
}
#[derive(Debug, thiserror::Error)]
pub enum RootPreparationError {
    #[error("root-entry mount namespace: {0}")]
    Namespace(#[from] NamespaceFailure),
    #[error("restricted root preparation: {0}")]
    Restrict(#[from] io::Error),
}
/// Restricted leaf retaining only its start endpoint. Its construction and
/// start are exit-only on failure, in the dedicated trusted PID-1 helper.
#[derive(Debug)]
pub struct GatedRestrictedCommand {
    command: RestrictedCommand,
    gate: ExecutionEndpoint,
}
impl GatedRootSetup {
    /// Select the assigned cwd, install private runtime mounts, pivot root,
    /// restrict privileges/syscalls, and retain the explicit start handshake.
    /// No image command executes during this transition.
    pub fn restrict(self, script: &str) -> Result<GatedRestrictedCommand, RootPreparationError> {
        let (devices, gate) = self.into_parts();
        let scope = PrivateMountNamespace::enter()?;
        let root = CommandRoot::from_current_directory(scope, devices)?;
        let filter = CommandSyscalls::compile()?;
        let command = root
            .enter()?
            .restrict(script, CommandPrivileges::filesystem(), filter)?;
        Ok(GatedRestrictedCommand { command, gate })
    }
}
impl GatedRestrictedCommand {
    pub fn start(self, setup_deadline: Instant) -> io::Result<Infallible> {
        self.command.start(self.gate, setup_deadline)
    }
}
