// SPDX-License-Identifier: Apache-2.0
use super::CommandDomain;
use std::{fs::File, io};
/// Borrowing the live cgroup prevents consuming it for cancellation during joining.
#[derive(Debug)]
pub(crate) struct DomainJoin<'domain> {
    domain: &'domain CommandDomain,
}
impl CommandDomain {
    pub(crate) fn join_permit(&self) -> io::Result<DomainJoin<'_>> {
        self.target.lease.verify_current()?;
        Ok(DomainJoin { domain: self })
    }
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn name(&self) -> &std::ffi::OsStr {
        &self.target.name
    }
}
impl DomainJoin<'_> {
    pub(crate) fn join_file(&self) -> io::Result<File> {
        self.domain.join_file()
    }
}
