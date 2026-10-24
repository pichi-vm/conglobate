// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Owning waits for trusted helper children. Reaping one child does not prove
//! descendant quiescence, and this API does not admit untrusted build commands.
use std::{
    io,
    process::{Child, ExitStatus},
    time::{Duration, Instant},
};

mod threads;
pub(crate) use threads::ThreadSnapshot;
mod diagnostics;
pub(crate) use diagnostics::DiagnosticRelay;
mod syscalls;
pub(crate) use syscalls::CommandSyscalls;
mod privileges;
pub(crate) use privileges::CommandPrivileges;
mod reaper;
pub(crate) use reaper::{ChildReaper, ReapedChildren};
pub(crate) mod supervision;
pub(crate) use supervision::{Admitted, Joined, ObservedState, Paired, Supervision};
mod observation;
pub(crate) use observation::ProcessObservation;
pub(crate) mod identity;
#[cfg(test)]
pub(crate) use identity::IdentityReceiver;
mod launch;
pub(crate) use launch::LaunchFailure;

mod private {
    pub(crate) trait Sealed {}
}
pub(crate) trait ProcessState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
/// Status has not yet been collected by this owner; not proof of OS liveness.
pub(crate) struct Running;
/// A trusted helper whose own pidfd was received over its private channel.
/// This state does not establish containment or authorize recipe execution.
///
#[derive(Debug)]
pub(crate) struct Identified;
#[derive(Debug)]
pub(crate) struct Reaped;
impl private::Sealed for Running {}
impl private::Sealed for Identified {}
impl private::Sealed for Reaped {}
impl ProcessState for Running {
    type Resources = ChildLease<WaitOwned>;
}
impl ProcessState for Identified {
    type Resources = ChildLease<Pinned>;
}
pub(crate) trait LiveState: ProcessState<Resources = ChildLease<Self::Identity>> {
    type Identity: Termination;
}
impl LiveState for Running {
    type Identity = WaitOwned;
}
impl LiveState for Identified {
    type Identity = Pinned;
}
impl ProcessState for Reaped {
    type Resources = ExitStatus;
}

/// A reaped state means wait collected this child's status, not that the child
/// succeeded or that all processes it could have created are gone.
#[derive(Debug)]
pub(crate) struct Process<S: ProcessState> {
    resources: S::Resources,
}
#[derive(Debug)]
pub(crate) struct ChildLease<I: Termination> {
    child: Option<Child>,
    termination: I,
}
/// Sealed termination authority carried by a live process's state payload.
pub(crate) trait Termination: private::Sealed + std::fmt::Debug {
    #[doc(hidden)]
    fn kill(&self, child: &mut Child) -> io::Result<()>;
    #[doc(hidden)]
    fn wait_until(&self, child: &Child, deadline: Instant) -> io::Result<()>;
}
#[derive(Debug)]
pub(crate) struct WaitOwned {
    _private: (),
}
#[derive(Debug)]
pub(crate) struct Pinned {
    observation: ProcessObservation,
}
impl private::Sealed for WaitOwned {}
impl private::Sealed for Pinned {}
impl Termination for WaitOwned {
    fn kill(&self, child: &mut Child) -> io::Result<()> {
        child.kill()
    }
    fn wait_until(&self, child: &Child, deadline: Instant) -> io::Result<()> {
        // The exclusive wait owner retains even an exited child's PID until
        // reaping. SIGCHLD must not be ignored or use SA_NOCLDWAIT; no other
        // thread/reaper may collect this child during its owned lifetime.
        let pid = i32::try_from(child.id())
            .ok()
            .and_then(rustix::process::Pid::from_raw)
            .ok_or_else(|| io::Error::other("invalid owned child PID"))?;
        ProcessObservation {
            identity: rustix::process::pidfd_open(pid, rustix::process::PidfdFlags::empty())?,
        }
        .wait_until(deadline)
    }
}
impl Termination for Pinned {
    fn kill(&self, _child: &mut Child) -> io::Result<()> {
        match rustix::process::pidfd_send_signal(
            &self.observation.identity,
            rustix::process::Signal::KILL,
        ) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
    fn wait_until(&self, _child: &Child, deadline: Instant) -> io::Result<()> {
        self.observation.wait_until(deadline)
    }
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum WaitError {
    #[error("child wait deadline expired")]
    Deadline,
    #[error("child process operation: {0}")]
    Io(#[from] io::Error),
}
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub(crate) struct WaitFailure<S: LiveState = Running> {
    error: WaitError,
    process: Process<S>,
}
impl<S: LiveState> WaitFailure<S> {
    #[cfg(test)]
    pub(crate) fn error(&self) -> &WaitError {
        &self.error
    }
    pub(crate) fn into_parts(self) -> (WaitError, Process<S>) {
        (self.error, self.process)
    }
}
impl From<Child> for Process<Running> {
    /// Transfer exclusive reaping ownership. SIGCHLD must retain waitable
    /// children (neither SIG_IGN nor SA_NOCLDWAIT); no other waiter may reap it.
    fn from(child: Child) -> Self {
        Self {
            resources: ChildLease {
                child: Some(child),
                termination: WaitOwned { _private: () },
            },
        }
    }
}
impl Process<Identified> {
    /// Resolve the exclusively wait-owned child in this procfs instance only
    /// after its stable pidfd reports that same PID in the instance's PID view.
    /// This internal handle supports namespace maps, not arbitrary PID access.
    pub(crate) fn proc_directory(&self) -> io::Result<std::fs::File> {
        use std::{io::Read, os::fd::AsRawFd};
        let identity = &self.resources.termination.observation.identity;
        let info =
            std::fs::File::open(format!("/proc/thread-self/fdinfo/{}", identity.as_raw_fd()))?;
        if rustix::fs::fstatfs(&info)?.f_type != 0x9fa0 {
            return Err(io::Error::other("process identity requires procfs"));
        }
        let mut text = String::new();
        info.take(4097).read_to_string(&mut text)?;
        let visible = text
            .lines()
            .find_map(|line| line.strip_prefix("Pid:\t"))
            .and_then(|value| value.trim().parse::<u32>().ok());
        if text.len() > 4096 || visible != Some(self.id()) {
            return Err(io::Error::other(
                "pidfd identity does not match procfs child PID",
            ));
        }
        let directory = std::fs::File::open(format!("/proc/{}", self.id()))?;
        if rustix::fs::fstatfs(&directory)?.f_type != 0x9fa0 {
            return Err(io::Error::other("child directory requires procfs"));
        }
        Ok(directory)
    }
}

impl<S: LiveState> Process<S> {
    /// Diagnostic PID only. Identified helpers are signaled through their pidfd.
    pub(crate) fn id(&self) -> u32 {
        self.resources.child.as_ref().expect("live child").id()
    }

    /// Trusted namespace-helper use only. The enclosing supervisor owns the
    /// execution deadline and cancels the entire containment domain. This wait
    /// must never be used by that supervisor in place of its bounded wait.
    pub(crate) fn wait_under_supervisor(mut self) -> Result<Process<Reaped>, WaitFailure<S>> {
        match self.resources.child.as_mut().expect("live child").wait() {
            Ok(status) => {
                self.resources.child.take();
                Ok(Process { resources: status })
            }
            Err(error) => Err(WaitFailure {
                error: error.into(),
                process: self,
            }),
        }
    }

    pub(crate) fn wait_until(
        mut self,
        deadline: Instant,
    ) -> Result<Process<Reaped>, WaitFailure<S>> {
        loop {
            match self
                .resources
                .child
                .as_mut()
                .expect("live child")
                .try_wait()
            {
                Ok(Some(status)) => {
                    self.resources.child.take();
                    return Ok(Process { resources: status });
                }
                Ok(None) => {}
                Err(error) => {
                    return Err(WaitFailure {
                        error: error.into(),
                        process: self,
                    });
                }
            }
            if let Err(error) = self
                .resources
                .termination
                .wait_until(self.resources.child.as_ref().expect("live child"), deadline)
            {
                return Err(WaitFailure {
                    error: if error.kind() == io::ErrorKind::TimedOut {
                        WaitError::Deadline
                    } else {
                        error.into()
                    },
                    process: self,
                });
            }
        }
    }

    /// Request termination, then collect status under the caller's teardown
    /// deadline. An error retains the child owner; it is never a reaped token.
    pub(crate) fn terminate_until(
        mut self,
        deadline: Instant,
    ) -> Result<Process<Reaped>, WaitFailure<S>> {
        let child = self.resources.child.as_mut().expect("live child");
        match child.try_wait() {
            Ok(Some(status)) => {
                self.resources.child.take();
                return Ok(Process { resources: status });
            }
            Err(error) => {
                return Err(WaitFailure {
                    error: error.into(),
                    process: self,
                });
            }
            Ok(None) => {}
        }
        if let Err(error) = self.resources.termination.kill(child) {
            return Err(WaitFailure {
                error: error.into(),
                process: self,
            });
        }
        self.wait_until(deadline)
    }
}
impl Process<Reaped> {
    pub(crate) fn status(&self) -> ExitStatus {
        self.resources
    }
}
impl<I: Termination> Drop for ChildLease<I> {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Best-effort fallback only. Never block indefinitely in Drop or
            // manufacture a quiescence receipt. The enclosing step must retain
            // resource authority for unresolved process lifetimes until VM exit.
            let _ = self.termination.kill(&mut child);
            let deadline = Instant::now() + Duration::from_millis(250);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => return,
                    Err(error) => {
                        eprintln!("helper reaping failed: {error}");
                        return;
                    }
                    Ok(None) => {}
                }
                if let Err(error) = self.termination.wait_until(&child, deadline) {
                    eprintln!("helper {} remains unreaped: {error}", child.id());
                    return;
                }
            }
        }
    }
}
