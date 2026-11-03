// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ResolverSource, ResolverView};
use std::{io, time::Instant};

/// Per-step resolver bytes retained until descendant quiescence.
#[derive(Debug)]
pub(crate) struct CommandNetwork {
    resolver: ResolverSource,
}
impl CommandNetwork {
    pub(crate) fn new(resolver: ResolverSource) -> Self {
        Self { resolver }
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
