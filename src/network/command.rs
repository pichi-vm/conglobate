// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{NetworkNamespace, ResolverSource, ResolverView};
use std::{io, time::Instant};

/// Per-step handle to environment networking and privately staged resolver bytes.
/// Retained until descendant quiescence. Closing never changes the external network.
#[derive(Debug)]
pub(crate) struct CommandNetwork {
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
    pub(crate) fn namespace(&self) -> &NetworkNamespace {
        &self.namespace
    }
    pub(crate) fn resolver_view(&self) -> io::Result<ResolverView> {
        self.resolver.view()
    }
    pub(crate) fn close(self, deadline: Instant) -> io::Result<()> {
        if Instant::now() >= deadline {
            std::mem::forget(self);
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "network cleanup deadline expired",
            ));
        }
        self.resolver.close()
    }
}
