// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
};
use std::{io, time::Instant};

/// Stable kernel identity for exit observation, independent of the child's
/// wait owner. No signaling, namespace entry or raw descriptor authority is
/// exposed. Exit observation is not a descendant-quiescence receipt.
#[derive(Debug)]
pub(crate) struct ProcessObservation {
    pub(super) identity: OwnedFd,
}
impl ProcessObservation {
    /// Wait for thread-group exit through the stable pidfd. Channel closure is
    /// not exit. This borrows observation only: the wait owner must still reap
    /// the process, and its descendants must still be cancelled and drained.
    pub(crate) fn wait_until(&self, deadline: Instant) -> io::Result<()> {
        loop {
            if self.has_exited()? {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "process exit deadline expired",
                ));
            }
            let timeout = Timespec {
                tv_sec: remaining
                    .as_secs()
                    .try_into()
                    .map_err(|_| io::Error::other("process exit deadline too distant"))?,
                tv_nsec: remaining.subsec_nanos().into(),
            };
            let mut descriptors = [PollFd::new(&self.identity, PollFlags::IN)];
            match poll(&mut descriptors, Some(&timeout)) {
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Sample whether the entire thread group has exited, without waiting or
    /// collecting its status. Remains true after the wait owner reaps it.
    /// Syscall errors remain errors, never evidence of exit or liveness.
    pub(crate) fn has_exited(&self) -> io::Result<bool> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        process::{Command, Stdio},
        time::Duration,
    };

    #[test]
    fn exit_wait_times_out_then_observes_exit_without_reaping() {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "read line || exit 0"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let observation = ProcessObservation {
            identity: rustix::process::pidfd_open(
                rustix::process::Pid::from_raw(child.id() as i32).unwrap(),
                rustix::process::PidfdFlags::empty(),
            )
            .unwrap(),
        };
        assert_eq!(
            observation.wait_until(Instant::now()).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(child.try_wait().unwrap().is_none());
        drop(input);
        observation
            .wait_until(Instant::now() + Duration::from_secs(5))
            .unwrap();
        assert!(child.wait().unwrap().success());
        observation.wait_until(Instant::now()).unwrap();
    }
}
