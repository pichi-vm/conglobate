// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Paired, Supervision};
use crate::process::identity::{ExchangeWait, IdentitySocket};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use std::{io, time::Instant};

#[derive(Debug)]
pub(in crate::process) struct StartupWatch<'scope> {
    supervisor: &'scope Supervision<Paired>,
    deadline: Instant,
}
impl<'scope> StartupWatch<'scope> {
    pub(in crate::process) fn new(
        supervisor: &'scope Supervision<Paired>,
        deadline: Instant,
    ) -> Self {
        Self {
            supervisor,
            deadline,
        }
    }
    pub(in crate::process) fn check(&self) -> io::Result<()> {
        if self.supervisor.peer_lost()? {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "supervisor lost during helper setup",
            ));
        }
        Ok(())
    }
}
impl ExchangeWait for StartupWatch<'_> {
    fn ready(&self, socket: &IdentitySocket, interest: PollFlags) -> io::Result<()> {
        loop {
            self.check()?;
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "helper setup deadline expired",
                ));
            }
            let timeout = Timespec {
                tv_sec: remaining
                    .as_secs()
                    .try_into()
                    .map_err(|_| io::Error::other("setup deadline too distant"))?,
                tv_nsec: remaining.subsec_nanos().into(),
            };
            let mut descriptors = [
                PollFd::new(&socket.0, interest),
                PollFd::new(&self.supervisor.socket.0.0, PollFlags::IN),
                PollFd::new(&self.supervisor.peer.identity, PollFlags::IN),
            ];
            match poll(&mut descriptors, Some(&timeout)) {
                Ok(0) | Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
                Ok(_) => {
                    // Upstream loss wins even when the helper is also ready.
                    self.check()?;
                    if !descriptors[0].revents().is_empty() {
                        return Ok(());
                    }
                }
            }
        }
    }
}
