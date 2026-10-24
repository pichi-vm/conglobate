// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Running, Step, StepState, StepStorage};
use std::{
    io,
    time::{Duration, Instant},
};

pub(super) const TEARDOWN_TIMEOUT: Duration = Duration::from_secs(30);
#[derive(Debug)]
pub(super) struct Released<R: StepStorage> {
    pub status: std::process::ExitStatus,
    pub reaped: crate::process::ReapedChildren,
    pub storage: R,
}
/// Quiescent descendants and released command namespace; storage still needs unmount.
#[derive(Debug)]
pub(crate) struct TeardownCompletion<R: StepStorage> {
    pub(super) released: Released<R>,
    pub(super) deadline: Instant,
}
#[derive(Debug)]
pub(super) struct Deadline;
impl Deadline {
    pub(super) fn check(deadline: Instant) -> io::Result<()> {
        if Instant::now() >= deadline {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "fixed step teardown deadline expired",
            ))
        } else {
            Ok(())
        }
    }
}
impl<'a, S: StepState<'a>, R: StepStorage> Step<'a, S, R> {
    pub(crate) fn teardown(self) -> io::Result<TeardownCompletion<R>> {
        self.teardown_until(Instant::now() + TEARDOWN_TIMEOUT)
    }
    pub(super) fn teardown_until(self, deadline: Instant) -> io::Result<TeardownCompletion<R>> {
        // Retained storage and namespace are not released on any failed check.
        let (status, reaped) = S::cleanup(self.resources, deadline)?;
        Deadline::check(deadline)?;
        drop(std::mem::ManuallyDrop::into_inner(self.namespace));
        if let Some(network) = std::mem::ManuallyDrop::into_inner(self.network) {
            network.close(deadline)?;
        }
        let storage = self.storage.release();
        Ok(TeardownCompletion {
            released: Released {
                status,
                reaped,
                storage,
            },
            deadline,
        })
    }
}
impl<'a, R: StepStorage> Step<'a, Running, R> {
    pub(super) fn teardown_after_command(
        self,
        deadline: Instant,
        status: std::process::ExitStatus,
    ) -> io::Result<TeardownCompletion<R>> {
        // A missing cleanup acknowledgment must not prevent cancellation/drain.
        let acknowledged = self.supervision().cleanup_until(status, deadline);
        let cleaned = self.teardown_until(deadline);
        match (acknowledged, cleaned) {
            (Ok(()), result) => result,
            (Err(error), Ok(completion)) => {
                // Never release mounted storage on an uncertified cleanup path.
                std::mem::forget(completion);
                Err(error)
            }
            (Err(primary), Err(cleanup)) => Err(io::Error::new(
                primary.kind(),
                format!("{primary}; cancellation: {cleanup}"),
            )),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_budget_and_expiry_are_checked_without_sleeping() {
        assert_eq!(TEARDOWN_TIMEOUT, Duration::from_secs(30));
        assert_eq!(
            Deadline::check(Instant::now()).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(Deadline::check(Instant::now() + TEARDOWN_TIMEOUT).is_ok());
    }
}
