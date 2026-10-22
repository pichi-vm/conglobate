// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use nix::{
    errno::Errno,
    sys::{
        prctl,
        wait::{WaitPidFlag, WaitStatus, waitpid},
    },
    unistd::{Pid, getpid},
};
use std::{
    fs, io,
    marker::PhantomData,
    rc::Rc,
    time::{Duration, Instant},
};

/// Process-lifetime adoption in a dedicated, single-threaded guardian. Install
/// before spawning children, with normal SIGCHLD zombie semantics and exclusive
/// wait ownership. Trusted code must not change these settings or fork this owner.
/// Drop deliberately leaves subreaping enabled: disabling it cannot clean children.
#[derive(Debug)]
pub struct ChildReaper {
    process: Pid,
    collected: u64,
    _thread: PhantomData<Rc<()>>,
}
/// Diagnostic count followed by an ECHILD observation, not a sandbox completion
/// receipt. Admission must be closed and containment independently quiescent.
#[derive(Debug)]
pub struct ReapedChildren {
    collected: u64,
}
impl ReapedChildren {
    pub fn collected(&self) -> u64 {
        self.collected
    }
}
#[derive(Debug, thiserror::Error)]
#[error("guardian child drain: {source}")]
pub struct DrainFailure {
    source: io::Error,
    reaper: ChildReaper,
}
impl DrainFailure {
    pub fn into_parts(self) -> (io::Error, ChildReaper) {
        (self.source, self.reaper)
    }
}
impl ChildReaper {
    pub fn install() -> io::Result<Self> {
        if getpid().as_raw() == 1
            || fs::read_dir("/proc/self/task")?
                .collect::<io::Result<Vec<_>>>()?
                .len()
                != 1
        {
            return Err(io::Error::other(
                "subreaper requires a dedicated single-threaded non-init process",
            ));
        }
        if !fs::read_to_string("/proc/thread-self/children")?
            .trim()
            .is_empty()
            || prctl::get_child_subreaper()?
        {
            return Err(io::Error::other(
                "subreaper must be installed once before child creation",
            ));
        }
        prctl::set_child_subreaper(true)?;
        Ok(Self {
            process: getpid(),
            collected: 0,
            _thread: PhantomData,
        })
    }

    /// Call only after all individually owned child statuses are collected.
    /// This process-wide wait includes clone children (__WALL); it must never
    /// race another waiter. Timeout/errors retain cumulative count and ownership.
    /// No signaling is performed: containment cancellation is a separate gate.
    pub fn drain_until(mut self, deadline: Instant) -> Result<ReapedChildren, DrainFailure> {
        if let Err(source) = self.drain(deadline) {
            return Err(DrainFailure {
                source,
                reaper: self,
            });
        }
        Ok(ReapedChildren {
            collected: self.collected,
        })
    }
    fn drain(&mut self, deadline: Instant) -> io::Result<()> {
        if getpid() != self.process || !prctl::get_child_subreaper()? {
            return Err(io::Error::other(
                "guardian subreaper identity or setting changed",
            ));
        }
        loop {
            match waitpid(None, Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL)) {
                Err(Errno::ECHILD) => return Ok(()),
                Err(Errno::EINTR) | Ok(WaitStatus::StillAlive) => {}
                Err(error) => return Err(error.into()),
                Ok(WaitStatus::Exited(..) | WaitStatus::Signaled(..)) => {
                    self.collected = self
                        .collected
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("child drain count overflow"))?;
                }
                Ok(_) => {}
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "guardian children remain undrained",
                ));
            }
            std::thread::sleep(remaining.min(Duration::from_millis(5)));
        }
    }
}
