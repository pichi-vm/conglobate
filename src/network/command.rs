// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    ConfiguredVeth, ConnectionFailure, NatCloseFailure, NatPolicy, NetworkNamespace,
    ResolverSource, ResolverSourceFailure, ResolverView,
};
use crate::storage::FailureDiagnostics;
use std::{io, time::Instant};

/// Configured resources assigned to one command. The step must retain this
/// owner through descendant quiescence, including cancellation and failed setup.
#[derive(Debug)]
pub struct CommandNetwork {
    policy: NatPolicy,
    resolver: ResolverSource,
}
impl CommandNetwork {
    pub fn new(policy: NatPolicy, resolver: ResolverSource) -> Self {
        Self { policy, resolver }
    }
    pub fn namespace(&self) -> &NetworkNamespace {
        self.policy.namespace()
    }
    pub(crate) fn resolver_view(&self) -> io::Result<ResolverView> {
        self.resolver.view()
    }
    /// No commands may remain. Every transition and retry uses this original
    /// teardown cutoff; no interface is released before acknowledged NAT close.
    pub fn close(self, deadline: Instant) -> Result<(), Box<NetworkCleanupFailure>> {
        Cleanup {
            state: State::Policy(self.policy, self.resolver),
            deadline,
        }
        .run()
    }
}
#[derive(Debug)]
enum State {
    Policy(NatPolicy, ResolverSource),
    PolicyRetry(Box<NatCloseFailure>, ResolverSource),
    Connection(ConfiguredVeth, ResolverSource),
    ConnectionRetry(ConnectionFailure, ResolverSource),
    Resolver(ResolverSource),
    ResolverRetry(ResolverSourceFailure),
    Complete,
}
#[derive(Debug)]
struct Cleanup {
    state: State,
    deadline: Instant,
}
#[derive(Debug, thiserror::Error)]
#[error("command network cleanup: {diagnostics}")]
pub struct NetworkCleanupFailure {
    diagnostics: FailureDiagnostics,
    cleanup: Cleanup,
}
impl NetworkCleanupFailure {
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn retry(self) -> Result<(), Box<Self>> {
        self.cleanup.run().map_err(|failure| {
            let Self {
                diagnostics,
                cleanup,
            } = *failure;
            Box::new(Self {
                diagnostics: self.diagnostics.followed_by(diagnostics),
                cleanup,
            })
        })
    }
}
#[derive(Debug)]
struct TransitionFailure {
    source: FailureDiagnostics,
    state: State,
}
impl TransitionFailure {
    fn new(source: FailureDiagnostics, state: State) -> Box<Self> {
        Box::new(Self { source, state })
    }
}
impl Cleanup {
    fn failed(self, source: impl Into<FailureDiagnostics>) -> Box<NetworkCleanupFailure> {
        Box::new(NetworkCleanupFailure {
            diagnostics: source.into(),
            cleanup: self,
        })
    }
    fn run(self) -> Result<(), Box<NetworkCleanupFailure>> {
        if Instant::now() >= self.deadline {
            return Err(self.failed(io::Error::new(
                io::ErrorKind::TimedOut,
                "command network teardown deadline expired",
            )));
        }
        let deadline = self.deadline;
        let next = match self.state {
            State::Policy(policy, resolver) => match policy.close(deadline) {
                Ok(connection) => Ok(State::Connection(connection, resolver)),
                Err(failure) => Err(TransitionFailure::new(
                    failure.diagnostics().latest_only(),
                    State::PolicyRetry(failure, resolver),
                )),
            },
            State::PolicyRetry(failure, resolver) => match failure.cleanup() {
                Ok(connection) => Ok(State::Connection(connection, resolver)),
                Err(failure) => Err(TransitionFailure::new(
                    failure.diagnostics().latest_only(),
                    State::PolicyRetry(failure, resolver),
                )),
            },
            State::Connection(connection, resolver) => match connection.close_until(deadline) {
                Ok(()) => Ok(State::Resolver(resolver)),
                Err(failure) => Err(TransitionFailure::new(
                    failure.diagnostics().latest_only(),
                    State::ConnectionRetry(failure, resolver),
                )),
            },
            State::ConnectionRetry(failure, resolver) => match failure.cleanup() {
                Ok(()) => Ok(State::Resolver(resolver)),
                Err(failure) => Err(TransitionFailure::new(
                    failure.diagnostics().latest_only(),
                    State::ConnectionRetry(failure, resolver),
                )),
            },
            State::Resolver(resolver) => match resolver.close() {
                Ok(()) => Ok(State::Complete),
                Err(failure) => Err(TransitionFailure::new(
                    failure.diagnostics().latest_only(),
                    State::ResolverRetry(failure),
                )),
            },
            State::ResolverRetry(failure) => match failure.cleanup() {
                Ok(()) => Ok(State::Complete),
                Err(failure) => Err(TransitionFailure::new(
                    failure.diagnostics().latest_only(),
                    State::ResolverRetry(failure),
                )),
            },
            State::Complete => return Ok(()),
        };
        match next {
            Ok(state) => Self { state, deadline }.run(),
            Err(failure) => {
                let TransitionFailure { source, state } = *failure;
                Err(Self { state, deadline }.failed(source))
            }
        }
    }
}
