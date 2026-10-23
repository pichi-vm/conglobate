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

#[derive(Debug, thiserror::Error)]
#[error("network namespace creator failed: {0}")]
struct NamespaceCreatorExit(std::process::ExitStatus);

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
    fn new(error: impl Into<FailureDiagnostics>, pending: Pending) -> Box<Self> {
        Box::new(Self {
            diagnostics: error.into(),
            pending,
            deadline: Instant::now() + Duration::from_secs(30),
        })
    }
    fn before(error: impl Into<FailureDiagnostics>) -> Box<Self> {
        Self::new(error, Pending::Complete)
    }
    fn namespace(failure: NetworkNamespaceFailure) -> Box<Self> {
        let (primary, pending) = match failure {
            NetworkNamespaceFailure::Launch(LaunchFailure::Started { source, process }) => {
                (source, Pending::Creator(process))
            }
            NetworkNamespaceFailure::Launch(LaunchFailure::Before(source))
            | NetworkNamespaceFailure::Input(source) => (source, Pending::Complete),
            NetworkNamespaceFailure::Capture { source, process } => {
                (source, Pending::IdentifiedCreator(process))
            }
            NetworkNamespaceFailure::Reap(failure) => {
                let (source, process) = failure.into_parts();
                (
                    io::Error::other(source),
                    Pending::IdentifiedCreator(process),
                )
            }
            NetworkNamespaceFailure::Exited(status) => (
                io::Error::other(NamespaceCreatorExit(status)),
                Pending::Complete,
            ),
        };
        Self::new(primary, pending)
    }
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn cleanup(self) -> Result<(), Box<Self>> {
        let result = match self.pending {
            Pending::Complete => return Ok(()),
            _ if Instant::now() >= self.deadline => return Err(Box::new(self)),
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
                return Err(NetworkSetupFailure::before(io::Error::other(format!(
                    "network-enabled commands require environment forwarding: {path}=1"
                ))));
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
        .map_err(|failure| {
            NetworkSetupFailure::new(
                io::Error::other(failure.to_string()),
                Pending::Veth(failure),
            )
        })?;
        let connection = pair
            .connect(namespace, budget.deadline())
            .map_err(|failure| {
                NetworkSetupFailure::new(
                    io::Error::other(failure.to_string()),
                    Pending::Connection(failure),
                )
            })?;
        let connection = connection
            .configure(host, peer, program, cutoff)
            .map_err(|failure| {
                NetworkSetupFailure::new(
                    io::Error::other(failure.to_string()),
                    Pending::Configuration(ConfigurationCleanupFailure::Worker(failure)),
                )
            })?;
        let policy = NatPolicy::install(connection, budget.deadline()).map_err(|failure| {
            NetworkSetupFailure::new(io::Error::other(failure.to_string()), Pending::Nat(failure))
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

#[cfg(test)]
mod tests {
    use super::{NamespaceCreatorExit, NetworkNamespaceFailure, NetworkSetupFailure};
    use crate::process::LaunchFailure;
    use std::{io, os::unix::process::ExitStatusExt, process::ExitStatus};

    #[test]
    fn early_network_errors_retain_errno_without_resource_acquisition() {
        for failure in [
            NetworkSetupFailure::before(io::Error::from_raw_os_error(13)),
            NetworkSetupFailure::namespace(NetworkNamespaceFailure::Input(
                io::Error::from_raw_os_error(13),
            )),
            NetworkSetupFailure::namespace(NetworkNamespaceFailure::Launch(LaunchFailure::Before(
                io::Error::from_raw_os_error(13),
            ))),
        ] {
            assert_eq!(failure.diagnostics().operation().raw_os_error(), Some(13));
            assert!(failure.diagnostics().cleanup().is_empty());
            failure.cleanup().unwrap();
        }
    }

    #[test]
    fn namespace_creator_exit_retains_status_as_typed_error_payload() {
        for raw in [7 << 8, 9] {
            let failure = NetworkSetupFailure::namespace(NetworkNamespaceFailure::Exited(
                ExitStatus::from_raw(raw),
            ));
            let source = failure
                .diagnostics()
                .operation()
                .get_ref()
                .unwrap()
                .downcast_ref::<NamespaceCreatorExit>()
                .unwrap();
            assert_eq!(source.0.into_raw(), raw);
            assert!(
                failure
                    .to_string()
                    .contains("network namespace creator failed")
            );
            failure.cleanup().unwrap();
        }
    }
}
