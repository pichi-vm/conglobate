// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::domain::{DomainCreateFailure, DomainTarget};
use std::{io, time::Instant};

/// A failed create owns at most one empty cgroup directory; no helper was admitted.
#[derive(Debug)]
pub(crate) struct DomainCreationRollback {
    target: Option<DomainTarget>,
}
#[derive(Debug, thiserror::Error)]
#[error("domain creation cleanup: {source}")]
pub(crate) struct DomainCreationCleanupFailure {
    source: io::Error,
    rollback: DomainCreationRollback,
}
impl DomainCreationCleanupFailure {}
impl DomainCreateFailure {
    pub(crate) fn into_rollback(self) -> (io::Error, DomainCreationRollback) {
        match self {
            Self::Before(source) => (source, DomainCreationRollback { target: None }),
            Self::Created { source, target } => (
                source,
                DomainCreationRollback {
                    target: Some(target),
                },
            ),
        }
    }
}
impl DomainCreationRollback {
    pub(crate) fn retry_until(
        self,
        deadline: Instant,
    ) -> Result<(), Box<DomainCreationCleanupFailure>> {
        if Instant::now() >= deadline {
            return Err(Box::new(DomainCreationCleanupFailure {
                source: io::Error::new(io::ErrorKind::TimedOut, "domain cleanup deadline expired"),
                rollback: self,
            }));
        }
        if let Some(target) = self.target {
            target.remove().map_err(|failure| {
                let (source, target) = failure.into_parts();
                Box::new(DomainCreationCleanupFailure {
                    source,
                    rollback: Self {
                        target: Some(target),
                    },
                })
            })?;
        }
        Ok(())
    }
}
