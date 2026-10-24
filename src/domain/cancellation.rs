// SPDX-License-Identifier: Apache-2.0
mod setup;
use super::{CommandDomain, DomainFailure};
use std::time::Instant;

/// Consuming cancellation closes admission; failure never returns join authority.
#[derive(Debug)]
pub(crate) struct DomainCancellation {
    domain: CommandDomain,
}
#[derive(Debug, thiserror::Error)]
#[error("domain cancellation: {source}")]
pub(crate) struct DomainCancelFailure {
    source: std::io::Error,
    cancellation: DomainCancellation,
}
impl DomainCancelFailure {
    pub(crate) fn into_parts(self) -> (std::io::Error, DomainCancellation) {
        (self.source, self.cancellation)
    }
}
impl CommandDomain {
    pub(crate) fn cancellation(self) -> DomainCancellation {
        DomainCancellation { domain: self }
    }
}
impl DomainCancellation {
    pub(crate) fn retry_until(self, deadline: Instant) -> Result<(), DomainCancelFailure> {
        self.domain
            .cancel_until(deadline)
            .map_err(|failure: DomainFailure| {
                let (source, domain) = failure.into_parts();
                DomainCancelFailure {
                    source,
                    cancellation: Self { domain },
                }
            })
    }
}
