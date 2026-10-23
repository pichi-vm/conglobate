// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{RecoveryDirectory, control_record::ControlRecords};
use crate::domain::{CommandDomain, DomainRecord};
use std::{io, path::Path};
impl RecoveryDirectory {
    /// Publish a verified live domain's record without replacement. The domain
    /// must belong to this lease. An error after publication can leave a complete
    /// record: fail and inspect under the lease, never assume nothing was written.
    pub fn save_domain(&self, domain: &CommandDomain) -> io::Result<DomainRecord> {
        let record = domain.record_for(self)?;
        ControlRecords::new(self).save(record.name(), &record.to_cbor())?;
        Ok(record)
    }
    /// Load data only. Reopening still verifies parent and domain identities.
    pub fn load_domain(&self, name: &str) -> io::Result<DomainRecord> {
        if !DomainRecord::valid_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid domain name",
            ));
        }
        let record = DomainRecord::try_from(ControlRecords::new(self).load(name)?.as_slice())?;
        if record.name() != name {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "domain record name mismatch",
            ));
        }
        Ok(record)
    }
    /// Remove a matching record only after its exact parent is verified and the
    /// named target is absent. Reaping completion remains a separate obligation.
    pub fn remove_domain_record(&self, parent: &Path, record: &DomainRecord) -> io::Result<()> {
        if self.load_domain(record.name())? != *record {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "domain record identity mismatch",
            ));
        }
        record.verify_absent(parent)?;
        ControlRecords::new(self).remove(record.name())
    }
}
