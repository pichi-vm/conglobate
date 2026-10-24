// SPDX-License-Identifier: Apache-2.0
mod setup;
use super::CommandDomain;
use std::time::Instant;

/// Consuming cancellation closes admission; failure never returns join authority.
#[derive(Debug)]
pub(crate) struct DomainCancellation {
    domain: CommandDomain,
}
impl CommandDomain {
    pub(crate) fn cancellation(self) -> DomainCancellation {
        DomainCancellation { domain: self }
    }
}
impl DomainCancellation {
    pub(crate) fn finish_until(self, deadline: Instant) -> std::io::Result<()> {
        self.domain.cancel_until(deadline).map_err(|failure| {
            let (source, domain) = failure.into_parts();
            std::mem::forget(domain);
            source
        })
    }
}
