// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{DomainCancellation, DomainRecordCleanup, RecordDomainFailure, RecordedDomain};
use crate::{
    anchor::RecoveryDirectory,
    domain::{CommandDomain, DomainCreateFailure, DomainTarget},
};
use std::{io, path::Path, time::Instant};

#[derive(Debug)]
enum Pending<'l> {
    Complete,
    Target(DomainTarget),
    Inspect(CommandDomain),
    Unpublished(CommandDomain),
    Published(DomainCancellation<'l>),
    Record(DomainRecordCleanup<'l>),
}
/// Cleanup-only ownership of a domain which never admitted a helper. A failed
/// publication can have left either no record or the exact complete record.
/// An unrelated/malformed record is never removed on that assumption.
#[derive(Debug)]
pub struct DomainCreationRollback<'l> {
    pending: Pending<'l>,
    recovery: &'l RecoveryDirectory,
}
#[derive(Debug, thiserror::Error)]
#[error("domain creation rollback: {source}")]
pub struct DomainCreationCleanupFailure<'l> {
    source: io::Error,
    rollback: DomainCreationRollback<'l>,
}
impl<'l> DomainCreationCleanupFailure<'l> {
    pub fn into_parts(self) -> (io::Error, DomainCreationRollback<'l>) {
        (self.source, self.rollback)
    }
}
impl RecordDomainFailure {
    pub fn into_rollback(
        self,
        recovery: &RecoveryDirectory,
    ) -> (io::Error, DomainCreationRollback<'_>) {
        let (source, pending) = match self {
            Self::Create(DomainCreateFailure::Before(source)) => (source, Pending::Complete),
            Self::Create(DomainCreateFailure::Created { source, target }) => {
                (source, Pending::Target(target))
            }
            Self::Publication { source, domain } => (source, Pending::Inspect(domain)),
        };
        (source, DomainCreationRollback { pending, recovery })
    }
}
impl<'l> DomainCreationRollback<'l> {
    pub fn retry_until(
        self,
        parent: &Path,
        deadline: Instant,
    ) -> Result<(), Box<DomainCreationCleanupFailure<'l>>> {
        if Instant::now() >= deadline {
            return Err(Box::new(DomainCreationCleanupFailure {
                source: io::Error::new(io::ErrorKind::TimedOut, "domain rollback deadline expired"),
                rollback: self,
            }));
        }
        let Self { pending, recovery } = self;
        let next = match pending {
            Pending::Complete => return Ok(()),
            Pending::Target(target) => match target.remove() {
                Ok(()) => Ok(Pending::Complete),
                Err(failure) => {
                    let (error, target) = failure.into_parts();
                    Err((error, Pending::Target(target)))
                }
            },
            Pending::Inspect(domain) => {
                let record = domain.record_for(recovery).and_then(|expected| {
                    match recovery.load_domain(expected.name()) {
                        Ok(actual) if actual == expected => Ok(Some(expected)),
                        Ok(_) => Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "failed publication record identity mismatch",
                        )),
                        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
                        Err(error) => Err(error),
                    }
                });
                match record {
                    Ok(Some(record)) => Ok(Pending::Published(
                        RecordedDomain {
                            domain,
                            record,
                            recovery,
                        }
                        .cancellation(),
                    )),
                    Ok(None) => Ok(Pending::Unpublished(domain)),
                    Err(error) => Err((error, Pending::Inspect(domain))),
                }
            }
            Pending::Unpublished(domain) => match domain.cancel_until(deadline) {
                Ok(()) => Ok(Pending::Complete),
                Err(failure) => {
                    let (error, domain) = failure.into_parts();
                    Err((error, Pending::Unpublished(domain)))
                }
            },
            Pending::Published(domain) => match domain.retry_until(deadline) {
                Ok(record) => Ok(Pending::Record(record)),
                Err(failure) => {
                    let (error, domain) = failure.into_parts();
                    Err((error, Pending::Published(domain)))
                }
            },
            Pending::Record(record) => match record.finish(parent) {
                Ok(()) => Ok(Pending::Complete),
                Err(failure) => {
                    let (error, record) = failure.into_parts();
                    Err((error, Pending::Record(record)))
                }
            },
        };
        match next {
            Ok(pending) => Self { pending, recovery }.retry_until(parent, deadline),
            Err((source, pending)) => Err(Box::new(DomainCreationCleanupFailure {
                source,
                rollback: Self { pending, recovery },
            })),
        }
    }
}
