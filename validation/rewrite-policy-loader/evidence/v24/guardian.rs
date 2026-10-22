// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Paired, Registered, Supervision};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use std::{io, time::Instant};

/// Sampled reason to begin teardown. This is not an ordering of peer exits,
/// command success, descendant quiescence or a cleanup receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownTrigger {
    SupervisorLost,
    HelperLost,
    BothLost,
}

/// Registered-phase guardian observation. Borrows both channels so their owners
/// remain available for cancellation/recovery. Setup-phase and resource ownership
/// orchestration are separate obligations; errors also require failed teardown.
///
/// ```compile_fail
/// use conglobate::process::{GuardianWatch, Supervision, Paired};
/// fn cannot_watch_an_unregistered_helper(supervisor: &Supervision<Paired>, helper: &Supervision<Paired>) {
///     GuardianWatch::new(supervisor, helper);
/// }
/// ```
#[derive(Debug)]
pub struct GuardianWatch<'scope> {
    supervisor: &'scope Supervision<Paired>,
    helper: &'scope Supervision<Registered>,
}
impl<'scope> GuardianWatch<'scope> {
    pub fn new(
        supervisor: &'scope Supervision<Paired>,
        helper: &'scope Supervision<Registered>,
    ) -> Self {
        Self { supervisor, helper }
    }
    /// Wait on both channels and both stable process identities under one bound.
    /// A timeout does not certify either peer remains alive after the sample.
    pub fn wait_until(&self, deadline: Instant) -> io::Result<ShutdownTrigger> {
        loop {
            match (self.supervisor.peer_lost()?, self.helper.peer_lost()?) {
                (true, true) => return Ok(ShutdownTrigger::BothLost),
                (true, false) => return Ok(ShutdownTrigger::SupervisorLost),
                (false, true) => return Ok(ShutdownTrigger::HelperLost),
                (false, false) => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "guardian observation deadline expired",
                ));
            }
            let timeout = Timespec {
                tv_sec: remaining
                    .as_secs()
                    .try_into()
                    .map_err(|_| io::Error::other("guardian deadline too distant"))?,
                tv_nsec: remaining.subsec_nanos().into(),
            };
            let mut descriptors = [
                PollFd::new(&self.supervisor.socket.0.0, PollFlags::IN),
                PollFd::new(&self.supervisor.peer.identity, PollFlags::IN),
                PollFd::new(&self.helper.socket.0.0, PollFlags::IN),
                PollFd::new(&self.helper.peer.identity, PollFlags::IN),
            ];
            match poll(&mut descriptors, Some(&timeout)) {
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
}
