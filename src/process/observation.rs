// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    process::{Pid, PidfdFlags, pidfd_open},
};
use std::io;

/// Stable kernel identity for exit observation, independent of the child's
/// wait owner. No signaling, namespace entry or raw descriptor authority is
/// exposed. Exit observation is not a descendant-quiescence receipt.
#[derive(Debug)]
pub struct ProcessObservation {
    identity: OwnedFd,
}
impl ProcessObservation {
    pub(super) fn open(pid: Pid) -> io::Result<Self> {
        Ok(Self {
            identity: pidfd_open(pid, PidfdFlags::empty())?,
        })
    }

    /// Sample whether the entire thread group has exited, without waiting or
    /// collecting its status. Remains true after the wait owner reaps it.
    /// Syscall errors remain errors, never evidence of exit or liveness.
    pub fn has_exited(&self) -> io::Result<bool> {
        let mut fds = [PollFd::new(&self.identity, PollFlags::IN)];
        poll(
            &mut fds,
            Some(&Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }),
        )?;
        let events = fds[0].revents();
        if events.intersects(PollFlags::ERR | PollFlags::NVAL) {
            return Err(io::Error::other("invalid process observation readiness"));
        }
        Ok(events.intersects(PollFlags::IN | PollFlags::HUP))
    }
}
