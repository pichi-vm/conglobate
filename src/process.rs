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

mod private {
    pub trait Sealed {}
}
pub trait ProcessState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
/// Status has not yet been collected by this owner; not proof of OS liveness.
pub struct Running;
#[derive(Debug)]
pub struct Reaped;
impl private::Sealed for Running {}
impl private::Sealed for Reaped {}
impl ProcessState for Running {
    type Resources = ChildLease;
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
pub struct ChildLease {
    child: Option<Child>,
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
pub struct WaitFailure {
    error: WaitError,
    process: Process<Running>,
}
impl WaitFailure {
    pub fn error(&self) -> &WaitError {
        &self.error
    }
    pub fn into_parts(self) -> (WaitError, Process<Running>) {
        (self.error, self.process)
    }
}
impl From<Child> for Process<Running> {
    fn from(child: Child) -> Self {
        Self {
            resources: ChildLease { child: Some(child) },
        }
    }
}
impl Process<Running> {
    /// Diagnostic PID only. Signaling uses Child's retained wait relationship
    /// or recorded exit status, never an independently reopened numeric PID.
    pub fn id(&self) -> u32 {
        self.resources.child.as_ref().expect("live child").id()
    }

    pub fn wait_until(mut self, deadline: Instant) -> Result<Process<Reaped>, WaitFailure> {
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
    pub fn terminate_until(mut self, deadline: Instant) -> Result<Process<Reaped>, WaitFailure> {
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
        if let Err(error) = child.kill() {
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
impl Drop for ChildLease {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Best-effort fallback only. Never block indefinitely in Drop or
            // manufacture a quiescence receipt. The guardian/recovery protocol
            // must retain resource authority for unresolved process lifetimes.
            let _ = child.kill();
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
