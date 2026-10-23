// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use std::{fmt, io};

/// The original operation failure followed by failed cleanup attempts in order.
/// Errors retain their concrete payloads and OS error codes. Access is read-only;
/// retry transitions consume and extend the history while retaining resources.
#[derive(Debug)]
pub struct FailureDiagnostics {
    operation: io::Error,
    cleanup: Vec<io::Error>,
}
impl From<io::Error> for FailureDiagnostics {
    fn from(operation: io::Error) -> Self {
        Self {
            operation,
            cleanup: Vec::new(),
        }
    }
}
impl From<rustix::io::Errno> for FailureDiagnostics {
    fn from(error: rustix::io::Errno) -> Self {
        io::Error::from(error).into()
    }
}
impl FailureDiagnostics {
    pub fn operation(&self) -> &io::Error {
        &self.operation
    }
    pub fn cleanup(&self) -> &[io::Error] {
        &self.cleanup
    }
    pub fn latest(&self) -> &io::Error {
        self.cleanup.last().unwrap_or(&self.operation)
    }
    pub(super) fn followed_by(mut self, next: Self) -> Self {
        self.cleanup.push(next.operation);
        self.cleanup.extend(next.cleanup);
        self
    }
}
impl fmt::Display for FailureDiagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.operation)?;
        for (index, error) in self.cleanup.iter().enumerate() {
            write!(f, "; cleanup attempt {}: {error}", index + 1)?;
        }
        Ok(())
    }
}
impl std::error::Error for FailureDiagnostics {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.operation)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_preserve_original_codes_and_order_without_nested_wrappers() {
        let original = FailureDiagnostics::from(io::Error::from_raw_os_error(22));
        let retry = FailureDiagnostics::from(io::Error::from_raw_os_error(16))
            .followed_by(io::Error::from_raw_os_error(5).into());
        let combined = original.followed_by(retry);
        assert_eq!(combined.operation().raw_os_error(), Some(22));
        assert_eq!(
            combined
                .cleanup()
                .iter()
                .map(io::Error::raw_os_error)
                .collect::<Vec<_>>(),
            [Some(16), Some(5)]
        );
        assert_eq!(combined.latest().raw_os_error(), Some(5));
        assert!(combined.to_string().contains("cleanup attempt 2:"));
    }
}
