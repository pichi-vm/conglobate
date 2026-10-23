// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{NetworkNamespace, ResolverSource, ResolverSourceFailure, ResolverView};
use crate::storage::FailureDiagnostics;
use std::{io, time::Instant};

/// Per-step handle to environment networking and privately staged resolver bytes.
/// Retained until descendant quiescence. Closing never changes the external network.
#[derive(Debug)]
pub struct CommandNetwork {
    namespace: NetworkNamespace,
    resolver: ResolverSource,
}
impl CommandNetwork {
    pub(crate) fn new(namespace: NetworkNamespace, resolver: ResolverSource) -> Self {
        Self {
            namespace,
            resolver,
        }
    }
    pub fn namespace(&self) -> &NetworkNamespace {
        &self.namespace
    }
    pub(crate) fn resolver_view(&self) -> io::Result<ResolverView> {
        self.resolver.view()
    }
    pub fn close(self, deadline: Instant) -> Result<(), Box<NetworkCleanupFailure>> {
        Cleanup {
            namespace: self.namespace,
            resolver: Resolver::Source(self.resolver),
            deadline,
        }
        .run()
    }
}
#[derive(Debug)]
enum Resolver {
    Source(ResolverSource),
    Failed(ResolverSourceFailure),
}
#[derive(Debug)]
struct Cleanup {
    namespace: NetworkNamespace,
    resolver: Resolver,
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
            Box::new(Self {
                diagnostics: self
                    .diagnostics
                    .followed_by(failure.diagnostics.latest_only()),
                cleanup: failure.cleanup,
            })
        })
    }
}
impl Cleanup {
    fn run(self) -> Result<(), Box<NetworkCleanupFailure>> {
        if Instant::now() >= self.deadline {
            return Err(Box::new(NetworkCleanupFailure {
                diagnostics: io::Error::new(
                    io::ErrorKind::TimedOut,
                    "resolver source cleanup deadline expired",
                )
                .into(),
                cleanup: self,
            }));
        }
        let result = match self.resolver {
            Resolver::Source(source) => source.close(),
            Resolver::Failed(failure) => failure.cleanup(),
        };
        result.map_err(|failure| {
            Box::new(NetworkCleanupFailure {
                diagnostics: failure.diagnostics().clone(),
                cleanup: Self {
                    namespace: self.namespace,
                    resolver: Resolver::Failed(failure),
                    deadline: self.deadline,
                },
            })
        })
    }
}
