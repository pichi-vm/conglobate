// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Owning waits for trusted helper children. Reaping one child does not prove
//! descendant quiescence, and this API does not admit untrusted build commands.
use std::{
    io,
    process::{Child, ExitStatus},
    time::{Duration, Instant},
};

mod observation;
pub use observation::ProcessObservation;
mod identity;
pub use identity::{IdentityReceiver, IdentitySender};
mod launch;
pub use launch::LaunchFailure;

mod private {
    pub trait Sealed {}
}
pub trait ProcessState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
/// Status has not yet been collected by this owner; not proof of OS liveness.
pub struct Running;
/// A trusted helper whose own pidfd was received over its private channel.
/// This state does not establish containment or authorize recipe execution.
///
/// ```compile_fail
/// use conglobate::process::{Identified, Process};
/// use std::process::Child;
/// fn cannot_assert_identity(child: Child) -> Process<Identified> {
///     child.into()
/// }
/// ```
#[derive(Debug)]
pub struct Identified;
#[derive(Debug)]
pub struct Reaped;
impl private::Sealed for Running {}
impl private::Sealed for Identified {}
impl private::Sealed for Reaped {}
impl ProcessState for Running {
    type Resources = ChildLease<WaitOwned>;
}
impl ProcessState for Identified {
    type Resources = ChildLease<Pinned>;
}
pub trait LiveState: ProcessState<Resources = ChildLease<Self::Identity>> {
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
pub struct Process<S: ProcessState> {
    resources: S::Resources,
}
#[derive(Debug)]
pub struct ChildLease<I: Termination> {
    child: Option<Child>,
    termination: I,
}
/// Sealed termination authority carried by a live process's state payload.
pub trait Termination: private::Sealed + std::fmt::Debug {
    #[doc(hidden)]
    fn kill(&self, child: &mut Child) -> io::Result<()>;
}
#[derive(Debug)]
pub struct WaitOwned {
    _private: (),
}
#[derive(Debug)]
pub struct Pinned {
    observation: ProcessObservation,
}
impl private::Sealed for WaitOwned {}
impl private::Sealed for Pinned {}
impl Termination for WaitOwned {
    fn kill(&self, child: &mut Child) -> io::Result<()> {
        child.kill()
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
}
#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    #[error("child wait deadline expired")]
    Deadline,
    #[error("child process operation: {0}")]
    Io(#[from] io::Error),
}
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct WaitFailure<S: LiveState = Running> {
    error: WaitError,
    process: Process<S>,
}
impl<S: LiveState> WaitFailure<S> {
    pub fn error(&self) -> &WaitError {
        &self.error
    }
    pub fn into_parts(self) -> (WaitError, Process<S>) {
        (self.error, self.process)
    }
}
impl From<Child> for Process<Running> {
    fn from(child: Child) -> Self {
        Self {
            resources: ChildLease {
                child: Some(child),
                termination: WaitOwned { _private: () },
            },
        }
    }
}
impl<S: LiveState> Process<S> {
    /// Diagnostic PID only. Identified helpers are signaled through their pidfd.
    pub fn id(&self) -> u32 {
        self.resources.child.as_ref().expect("live child").id()
    }

    pub fn wait_until(mut self, deadline: Instant) -> Result<Process<Reaped>, WaitFailure<S>> {
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
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(WaitFailure {
                    error: WaitError::Deadline,
                    process: self,
                });
            }
            std::thread::sleep(remaining.min(Duration::from_millis(5)));
        }
    }

    /// Request termination, then collect status under the caller's teardown
    /// deadline. An error retains the child owner; it is never a reaped token.
    pub fn terminate_until(mut self, deadline: Instant) -> Result<Process<Reaped>, WaitFailure<S>> {
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
    pub fn status(&self) -> ExitStatus {
        self.resources
    }
}
impl<I: Termination> Drop for ChildLease<I> {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Best-effort fallback only. Never block indefinitely in Drop or
            // manufacture a quiescence receipt. The guardian/recovery protocol
            // must retain resource authority for unresolved process lifetimes.
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
                if Instant::now() >= deadline {
                    eprintln!(
                        "helper {} still requires recovery after termination",
                        child.id()
                    );
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        }
    }
}
