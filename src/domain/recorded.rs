// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{CommandDomain, DomainCreateFailure, DomainRecord, DomainRegistration};
use crate::anchor::RecoveryDirectory;
use std::{io, path::Path, time::Instant};

/// Owns a domain and its published identity under a borrowed recovery lease.
/// Drop leaves both kernel domain and record for recovery. Only explicit
/// cancellation transitions to record cleanup; this is not a complete step.
///
/// ```compile_fail
/// use conglobate::domain::RecordedDomain;
/// use std::path::Path;
/// fn cannot_finish_a_live_domain(domain: RecordedDomain<'_>) {
///     domain.finish(Path::new("/unused"));
/// }
/// ```
#[derive(Debug)]
pub struct RecordedDomain<'lease> {
    domain: CommandDomain,
    record: DomainRecord,
    recovery: &'lease RecoveryDirectory,
}
#[derive(Debug, thiserror::Error)]
pub enum RecordDomainFailure {
    #[error("domain creation: {0}")]
    Create(DomainCreateFailure),
    #[error("domain record publication: {source}")]
    Publication {
        source: io::Error,
        domain: CommandDomain,
    },
}
#[derive(Debug, thiserror::Error)]
#[error("recorded domain cancellation: {source}")]
pub struct RecordedCancelFailure<'lease> {
    source: io::Error,
    cancellation: DomainCancellation<'lease>,
}
impl<'lease> RecordedCancelFailure<'lease> {
    pub fn into_parts(self) -> (io::Error, DomainCancellation<'lease>) {
        (self.source, self.cancellation)
    }
}
/// Cancellation has begun. This owner permits cleanup retry, never registration.
///
/// ```compile_fail
/// use conglobate::domain::DomainCancellation;
/// fn cannot_register_after_cancellation(owner: DomainCancellation<'_>) {
///     owner.registration();
/// }
/// ```
#[derive(Debug)]
pub struct DomainCancellation<'lease> {
    domain: RecordedDomain<'lease>,
}
/// Domain cancellation/removal completed; the protected record still needs
/// deletion. This says nothing about reaping, policy removal or image validity.
#[derive(Debug)]
pub struct DomainRecordCleanup<'lease> {
    record: DomainRecord,
    recovery: &'lease RecoveryDirectory,
}
#[derive(Debug, thiserror::Error)]
#[error("domain record cleanup: {source}")]
pub struct RecordCleanupFailure<'lease> {
    source: io::Error,
    cleanup: DomainRecordCleanup<'lease>,
}
impl<'lease> RecordCleanupFailure<'lease> {
    pub fn into_parts(self) -> (io::Error, DomainRecordCleanup<'lease>) {
        (self.source, self.cleanup)
    }
}
impl<'lease> RecordedDomain<'lease> {
    pub(super) fn cancellation(self) -> DomainCancellation<'lease> {
        DomainCancellation { domain: self }
    }

    /// Publication errors retain the live domain. A complete record may already
    /// exist after an error; fail and inspect rather than assuming no write.
    pub fn create(
        recovery: &'lease RecoveryDirectory,
        parent: &Path,
    ) -> Result<Self, RecordDomainFailure> {
        let domain =
            CommandDomain::create(recovery, parent).map_err(RecordDomainFailure::Create)?;
        match recovery.save_domain(&domain) {
            Ok(record) => Ok(Self {
                domain,
                record,
                recovery,
            }),
            Err(source) => Err(RecordDomainFailure::Publication { source, domain }),
        }
    }
    pub fn record(&self) -> &DomainRecord {
        &self.record
    }
    pub fn registration(&self) -> io::Result<DomainRegistration<'_>> {
        self.domain.registration()
    }
    /// Stop admission first. Failure retains the entire domain/record owner.
    /// Success moves to record cleanup without manufacturing a reaping receipt.
    pub fn cancel_until(
        self,
        deadline: Instant,
    ) -> Result<DomainRecordCleanup<'lease>, RecordedCancelFailure<'lease>> {
        self.cancellation().retry_until(deadline)
    }
}
impl<'lease> DomainCancellation<'lease> {
    /// Caller must have excluded the old owner before recovering its domain.
    pub fn recover(
        recovery: &'lease RecoveryDirectory,
        parent: &Path,
        name: &str,
    ) -> io::Result<Self> {
        let record = recovery.load_domain(name)?;
        let domain = CommandDomain::recover_record(recovery, parent, record.clone())?;
        Ok(Self {
            domain: RecordedDomain {
                domain,
                record,
                recovery,
            },
        })
    }

    pub fn retry_until(
        self,
        deadline: Instant,
    ) -> Result<DomainRecordCleanup<'lease>, RecordedCancelFailure<'lease>> {
        let RecordedDomain {
            domain,
            record,
            recovery,
        } = self.domain;
        match domain.cancel_until(deadline) {
            Ok(()) => Ok(DomainRecordCleanup { record, recovery }),
            Err(failure) => {
                let (source, domain) = failure.into_parts();
                Err(RecordedCancelFailure {
                    source,
                    cancellation: Self {
                        domain: RecordedDomain {
                            domain,
                            record,
                            recovery,
                        },
                    },
                })
            }
        }
    }
}
impl<'lease> DomainRecordCleanup<'lease> {
    /// The supplied parent is identity-checked against the stored record; a
    /// relocated parent may be supplied on retry. Error retains this owner.
    /// A post-unlink error may leave no record; absence alone is not successful
    /// finalization and requires inspection under the retained lease.
    pub fn finish(self, parent: &Path) -> Result<(), RecordCleanupFailure<'lease>> {
        match self.recovery.remove_domain_record(parent, &self.record) {
            Ok(()) => Ok(()),
            Err(source) => Err(RecordCleanupFailure {
                source,
                cleanup: self,
            }),
        }
    }
}
