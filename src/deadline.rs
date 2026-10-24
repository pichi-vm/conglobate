// SPDX-License-Identifier: Apache-2.0
use rustix::event::Timespec;
use std::{io, time::Instant};

/// A fixed local cutoff. Sampling remaining time never renews the budget.
/// Cross-process clock conversion belongs to SetupCutoff, not this type.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Deadline(Instant);
impl Deadline {
    pub(crate) fn at(cutoff: Instant) -> Self {
        Self(cutoff)
    }
    pub(crate) fn remaining(self, expired: &'static str) -> io::Result<Timespec> {
        let remaining = self
            .0
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, expired))?;
        Timespec::try_from(remaining).map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_cutoffs_fail_and_future_cutoffs_are_not_renewed() {
        assert_eq!(
            Deadline::at(Instant::now())
                .remaining("expired")
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        let cutoff = Instant::now() + std::time::Duration::from_secs(2);
        let first = Deadline::at(cutoff).remaining("expired").unwrap();
        let next = Deadline::at(cutoff).remaining("expired").unwrap();
        assert!(next <= first);
        assert!(
            first
                <= Timespec {
                    tv_sec: 2,
                    tv_nsec: 0
                }
        );
    }
}
