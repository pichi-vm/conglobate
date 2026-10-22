// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::DomainEvents;
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use std::{
    fs::File,
    io::{self, Read, Seek},
    time::Instant,
};

/// A single open cgroup.events description retains kernfs's event counter
/// between reads and poll. Notifications only trigger another state read.
#[derive(Debug)]
pub(super) struct DomainWatch(File);
impl DomainWatch {
    pub(super) fn new(events: File) -> Self {
        Self(events)
    }

    pub(super) fn wait_empty_until(mut self, deadline: Instant) -> io::Result<()> {
        loop {
            self.0.rewind()?;
            let mut text = String::new();
            (&mut self.0).take(4097).read_to_string(&mut text)?;
            if text.len() > 4096 {
                return Err(io::Error::other("oversized cgroup event value"));
            }
            if !DomainEvents::try_from(text.as_str())?.populated {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "command domain remains populated",
                ));
            }
            let timeout = Timespec {
                tv_sec: remaining
                    .as_secs()
                    .try_into()
                    .map_err(|_| io::Error::other("domain deadline too distant"))?,
                tv_nsec: remaining.subsec_nanos().into(),
            };
            // Ordinary IN is always ready on kernfs. Changed event counters
            // report PRI | ERR; ERR is therefore not an I/O failure here.
            let mut descriptors = [PollFd::new(&self.0, PollFlags::PRI)];
            match poll(&mut descriptors, Some(&timeout)) {
                Ok(_) => {
                    if descriptors[0]
                        .revents()
                        .intersects(PollFlags::NVAL | PollFlags::HUP)
                    {
                        return Err(io::Error::other("cgroup event descriptor unavailable"));
                    }
                }
                Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
}
