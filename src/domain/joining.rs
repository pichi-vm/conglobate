// SPDX-License-Identifier: Apache-2.0
use super::CommandDomain;
use std::{fs::File, io};
/// Borrowing the live cgroup prevents consuming it for cancellation during joining.
#[derive(Debug)]
pub struct DomainJoin<'domain> {
    domain: &'domain CommandDomain,
}
impl CommandDomain {
    pub fn join_permit(&self) -> io::Result<DomainJoin<'_>> {
        self.target.lease.verify_current()?;
        Ok(DomainJoin { domain: self })
    }
    pub fn name(&self) -> &std::ffi::OsStr {
        &self.target.name
    }
}
impl DomainJoin<'_> {
    pub(crate) fn join_file(&self) -> io::Result<File> {
        self.domain.join_file()
    }
}
