// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{CommandDomain, DomainRecord};
use std::{fs::File, io};

/// Borrowed registration capability for a live domain with a matching protected
/// recovery record. Neither creation nor use permits sandbox/recipe admission.
/// The borrow prevents consuming this owner for cancellation during exchange.
///
/// ```compile_fail
/// use conglobate::{domain::CommandDomain, process::{Supervision, Paired}};
/// use std::time::Instant;
/// fn cannot_register_an_unpersisted_owner(domain: &CommandDomain, channel: Supervision<Paired>) {
///     channel.register_helper_until(domain, Instant::now());
/// }
/// ```
#[derive(Debug)]
pub struct DomainRegistration<'domain> {
    domain: &'domain CommandDomain,
    record: DomainRecord,
}
impl CommandDomain {
    /// Require successful record publication before exposing registration. The
    /// shared lease permits this check after the directory facade was dropped.
    pub fn registration(&self) -> io::Result<DomainRegistration<'_>> {
        let recovery = self.target.lease.directory();
        let record = self.record_for(&recovery)?;
        if recovery.load_domain(record.name())? != record {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "domain recovery record mismatch",
            ));
        }
        Ok(DomainRegistration {
            domain: self,
            record,
        })
    }
}
impl DomainRegistration<'_> {
    /// Recheck immediately before handing off the writable control descriptor.
    /// Callers must not mutate recovery storage through out-of-protocol paths.
    pub(crate) fn registration_file(&self) -> io::Result<File> {
        let current = self.domain.registration()?;
        if current.record != self.record {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "domain identity changed before registration",
            ));
        }
        self.domain.registration_file()
    }
}
