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
use std::{fs, io, marker::PhantomData, rc::Rc, time::Instant};

/// Process-lifetime adoption in a dedicated, single-threaded supervisor. Install
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
    reaper: ChildReaper,
}
impl ReapedChildren {
    pub fn collected(&self) -> u64 {
        self.reaper.collected
    }
    /// Resume exclusive reaping for the next step without reinstalling the
    /// process-wide subreaper setting. Counts begin anew for that step.
    pub fn into_reaper(self) -> ChildReaper {
        ChildReaper {
            collected: 0,
            ..self.reaper
        }
    }
}
#[derive(Debug, thiserror::Error)]
#[error("supervisor child drain: {source}")]
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
        Ok(ReapedChildren { reaper: self })
    }
    fn drain(&mut self, deadline: Instant) -> io::Result<()> {
        if getpid() != self.process || !prctl::get_child_subreaper()? {
            return Err(io::Error::other(
                "supervisor subreaper identity or setting changed",
            ));
        }
        loop {
            match waitpid(None, Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL)) {
                Err(Errno::ECHILD) => return Ok(()),
                Err(Errno::EINTR) => continue,
                Ok(WaitStatus::StillAlive) => {}
                Err(error) => return Err(error.into()),
                Ok(WaitStatus::Exited(..) | WaitStatus::Signaled(..)) => {
                    self.collected = self
                        .collected
                        .checked_add(1)
                        .ok_or_else(|| io::Error::other("child drain count overflow"))?;
                    if Instant::now() >= deadline {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "child drain deadline expired",
                        ));
                    }
                    continue;
                }
                Ok(_) => {}
            }
            self.wait_for_child(deadline)?;
        }
    }

    fn wait_for_child(&self, deadline: Instant) -> io::Result<()> {
        use std::os::fd::AsRawFd;
        // This single-threaded owner is the sole reaper. An exited child stays
        // waitable, retaining its PID until the next waitpid collects it.
        let children = fs::read_to_string("/proc/thread-self/children")?;
        let raw = children
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<i32>().ok())
            .and_then(rustix::process::Pid::from_raw)
            .ok_or_else(|| io::Error::other("waitable children missing from procfs"))?;
        let observation = super::ProcessObservation {
            identity: rustix::process::pidfd_open(raw, rustix::process::PidfdFlags::empty())?,
        };
        // Like individually owned helpers, reject a different procfs PID view
        // before using a numeric proc identifier as a stable wait target.
        let info = fs::read_to_string(format!(
            "/proc/thread-self/fdinfo/{}",
            observation.identity.as_raw_fd()
        ))?;
        let visible = info
            .lines()
            .find_map(|line| line.strip_prefix("Pid:\t"))
            .and_then(|value| value.trim().parse::<i32>().ok());
        if visible != Some(raw.as_raw_nonzero().get()) {
            return Err(io::Error::other(
                "child pidfd does not match procfs PID view",
            ));
        }
        observation.wait_until(deadline)
    }
}
