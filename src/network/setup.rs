// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    CommandNetwork, ConfigurationCleanupFailure, ConnectionFailure, NatCloseFailure,
    NatInstallFailure, NatPolicy, NetworkNamespace, NetworkNamespaceFailure, NetworkTopology,
    ResolverInput, ResolverSource, VethFailure, VethPair,
};
use crate::{
    execution::HelperProgram,
    process::{Identified, LaunchFailure, Process, Running},
    step::SetupBudget,
    storage::{FailureDiagnostics, Workspace},
};
use std::{
    io,
    path::Path,
    time::{Duration, Instant},
};

/// Serialize topology observation and assignment on the network namespace inode.
/// Independent opens share the kernel lock even across different mount namespaces.
/// CLOEXEC prevents trusted setup helpers from extending the lease after exec.
/// Contention fails immediately; the original setup cutoff is never renewed.
#[derive(Debug)]
struct AllocationLease {
    _namespace: std::fs::File,
}
impl AllocationLease {
    fn acquire() -> io::Result<Self> {
        let namespace = std::fs::File::open("/proc/self/ns/net")?;
        rustix::fs::flock(
            &namespace,
            rustix::fs::FlockOperation::NonBlockingLockExclusive,
        )
        .map_err(|error| {
            io::Error::new(
                io::Error::from(error).kind(),
                format!("command network allocation lease: {error}"),
            )
        })?;
        Ok(Self {
            _namespace: namespace,
        })
    }
}

/// Failed setup retains the exact resources still requiring cleanup. It never
/// yields an admissible network, even after cleanup succeeds.
#[derive(Debug, thiserror::Error)]
#[error("command network setup: {diagnostics}")]
pub struct NetworkSetupFailure {
    diagnostics: FailureDiagnostics,
    pending: Pending,
    deadline: Instant,
}
#[derive(Debug)]
enum Pending {
    Complete,
    Creator(Process<Running>),
    IdentifiedCreator(Process<Identified>),
    Veth(VethFailure),
    Connection(ConnectionFailure),
    Configuration(ConfigurationCleanupFailure),
    Nat(Box<NatInstallFailure>),
    Policy(NatPolicy),
    PolicyRetry(Box<NatCloseFailure>),
}
impl NetworkSetupFailure {
    fn new(error: impl std::fmt::Display, pending: Pending) -> Box<Self> {
        Box::new(Self {
            diagnostics: io::Error::other(error.to_string()).into(),
            pending,
            deadline: Instant::now() + Duration::from_secs(30),
        })
    }
    fn before(error: impl std::fmt::Display) -> Box<Self> {
        Self::new(error, Pending::Complete)
    }
    fn namespace(failure: NetworkNamespaceFailure) -> Box<Self> {
        let primary = failure.to_string();
        let pending = match failure {
            NetworkNamespaceFailure::Launch(LaunchFailure::Started { process, .. }) => {
                Pending::Creator(process)
            }
            NetworkNamespaceFailure::Capture { process, .. } => Pending::IdentifiedCreator(process),
            NetworkNamespaceFailure::Reap(failure) => {
                Pending::IdentifiedCreator(failure.into_parts().1)
            }
            _ => Pending::Complete,
        };
        Self::new(primary, pending)
    }
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn cleanup(self) -> Result<(), Box<Self>> {
        if matches!(self.pending, Pending::Complete) {
            return Ok(());
        }
        if Instant::now() >= self.deadline {
            return Err(Box::new(self));
        }
        let result = match self.pending {
            Pending::Complete => unreachable!(),
            Pending::Creator(process) => process
                .terminate_until(self.deadline)
                .map(|_| ())
                .map_err(|failure| {
                    let (error, process) = failure.into_parts();
                    (error.to_string(), Pending::Creator(process))
                }),
            Pending::IdentifiedCreator(process) => process
                .terminate_until(self.deadline)
                .map(|_| ())
                .map_err(|failure| {
                    let (error, process) = failure.into_parts();
                    (error.to_string(), Pending::IdentifiedCreator(process))
                }),
            Pending::Veth(failure) => failure
                .cleanup()
                .map_err(|failure| (failure.to_string(), Pending::Veth(failure))),
            Pending::Connection(failure) => failure
                .cleanup()
                .map_err(|failure| (failure.to_string(), Pending::Connection(failure))),
            Pending::Configuration(failure) => match failure {
                ConfigurationCleanupFailure::Worker(failure) => failure
                    .cleanup()
                    .map_err(|failure| (failure.to_string(), Pending::Configuration(failure))),
                ConfigurationCleanupFailure::Network(failure) => failure
                    .cleanup()
                    .map_err(|failure| (failure.to_string(), Pending::Connection(failure))),
            },
            Pending::Nat(failure) => failure
                .cleanup()
                .map_err(|failure| (failure.to_string(), Pending::Nat(failure))),
            Pending::Policy(policy) => match policy.close(self.deadline) {
                Ok(connection) => connection
                    .close_until(self.deadline)
                    .map_err(|failure| (failure.to_string(), Pending::Connection(failure))),
                Err(failure) => Err((failure.to_string(), Pending::PolicyRetry(failure))),
            },
            Pending::PolicyRetry(failure) => match failure.cleanup() {
                Ok(connection) => connection
                    .close_until(self.deadline)
                    .map_err(|failure| (failure.to_string(), Pending::Connection(failure))),
                Err(failure) => Err((failure.to_string(), Pending::PolicyRetry(failure))),
            },
        };
        result.map_err(|(error, pending)| {
            Box::new(Self {
                diagnostics: self.diagnostics.followed_by(io::Error::other(error).into()),
                pending,
                deadline: self.deadline,
            })
        })
    }
}
/// Captured build environment used only for explicitly enabled RUN directives.
#[derive(Debug)]
pub struct CommandNetworking<'a> {
    resolver: &'a ResolverInput,
    workspace: &'a Workspace,
}
impl<'a> CommandNetworking<'a> {
    pub fn new(resolver: &'a ResolverInput, workspace: &'a Workspace) -> Self {
        Self {
            resolver,
            workspace,
        }
    }
    pub(crate) fn prepare(
        &self,
        parent: &Path,
        program: &HelperProgram,
        budget: &SetupBudget,
    ) -> Result<CommandNetwork, Box<NetworkSetupFailure>> {
        CommandNetwork::prepare(self.resolver, self.workspace, parent, program, budget)
    }
}
impl CommandNetwork {
    /// Construct one private dual-stack NAT network. The environment enables
    /// forwarding beforehand; conglobate never changes primary NIC settings.
    /// Caller owns exclusive child reaping and must retain the result in Step.
    pub fn prepare(
        resolver: &ResolverInput,
        workspace: &Workspace,
        parent: &Path,
        program: &HelperProgram,
        budget: &SetupBudget,
    ) -> Result<Self, Box<NetworkSetupFailure>> {
        let cutoff = budget.cutoff().map_err(NetworkSetupFailure::before)?;
        let _allocation = AllocationLease::acquire().map_err(NetworkSetupFailure::before)?;
        for path in [
            "/proc/sys/net/ipv4/ip_forward",
            "/proc/sys/net/ipv6/conf/all/forwarding",
        ] {
            let value = std::fs::read_to_string(path).map_err(NetworkSetupFailure::before)?;
            if value.trim() != "1" {
                return Err(NetworkSetupFailure::before(format!(
                    "network-enabled commands require environment forwarding: {path}=1"
                )));
            }
        }
        let allocation = NetworkTopology::capture(budget.deadline())
            .and_then(|topology| topology.allocate())
            .map_err(NetworkSetupFailure::before)?;
        let (host, peer) = allocation
            .addresses()
            .map_err(NetworkSetupFailure::before)?;
        let namespace =
            NetworkNamespace::create(program, cutoff).map_err(NetworkSetupFailure::namespace)?;
        let pair = VethPair::create(
            parent,
            allocation.host_name().clone(),
            allocation.peer_name().clone(),
            budget.deadline(),
        )
        .map_err(|failure| NetworkSetupFailure::new(failure.to_string(), Pending::Veth(failure)))?;
        let connection = pair
            .connect(namespace, budget.deadline())
            .map_err(|failure| {
                NetworkSetupFailure::new(failure.to_string(), Pending::Connection(failure))
            })?;
        let connection = connection
            .configure(host, peer, program, cutoff)
            .map_err(|failure| {
                NetworkSetupFailure::new(
                    failure.to_string(),
                    Pending::Configuration(ConfigurationCleanupFailure::Worker(failure)),
                )
            })?;
        let policy = NatPolicy::install(connection, budget.deadline()).map_err(|failure| {
            NetworkSetupFailure::new(failure.to_string(), Pending::Nat(failure))
        })?;
        let resolver = match ResolverSource::prepare(resolver, workspace) {
            Ok(resolver) => resolver,
            Err(error) => return Err(NetworkSetupFailure::new(error, Pending::Policy(policy))),
        };
        // Resolver creation is synchronous; Step checks the shared setup budget
        // again before admission and retains this owner on expiry.
        Ok(Self::new(policy, resolver))
    }
}
