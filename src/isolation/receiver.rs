// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{CommandSetup, ExecutionGate};
use crate::{
    filesystem::{Attached, InUse, Mount, RootLaunchFailure, RootWaitFailure},
    process::{Admitted, Joined, Supervision},
    step::SetupCutoff,
};
use std::{
    io,
    process::{Command, ExitStatus, Stdio},
    time::Instant,
};

/// Outer trusted helper, already contained and in its private mount namespace.
/// The supervisor retains the volume, mapped namespace and cancellation authority. Errors
/// are exit-only in this helper; returned resources remain owned for teardown.
#[derive(Debug)]
pub struct CommandReceiver<'ns> {
    channel: Supervision<Joined>,
    mount: Mount<'ns, Attached>,
    setup: CommandSetup,
}
#[derive(Debug, thiserror::Error)]
pub enum ReceiverFailure<'ns> {
    #[error("receiver preparation: {source}")]
    Before {
        source: io::Error,
        mount: Mount<'ns, Attached>,
    },
    #[error("receiver launch: {0}")]
    Launch(RootLaunchFailure<'ns>),
    #[error("receiver startup: {source}")]
    Startup {
        source: io::Error,
        mount: Mount<'ns, InUse>,
    },
    #[error("receiver command wait: {0}")]
    Wait(RootWaitFailure<'ns>),
    #[error("receiver diagnostic relay: {source}")]
    Diagnostics {
        source: io::Error,
        mount: Mount<'ns, Attached>,
    },
    #[error("receiver setup failed after clean view teardown: {0}")]
    Cleaned(io::Error),
    #[error("receiver setup failed: {source}; view cleanup: {cleanup}")]
    Cleanup {
        source: io::Error,
        cleanup: crate::filesystem::UnmountFailure<'ns>,
    },
}
/// Direct command helper reaped. The caller must close this view and finish
/// local namespace cleanup before reporting completion. Descendant quiescence
/// is still the supervisor's responsibility; this is not build success.
#[derive(Debug)]
pub struct ReceiverCompletion<'ns> {
    channel: Supervision<Admitted>,
    mount: Mount<'ns, Attached>,
    status: ExitStatus,
}
impl<'ns> ReceiverCompletion<'ns> {
    pub fn into_parts(self) -> (Supervision<Admitted>, Mount<'ns, Attached>, ExitStatus) {
        (self.channel, self.mount, self.status)
    }
}
impl<'ns> CommandReceiver<'ns> {
    pub fn new(
        channel: Supervision<Joined>,
        mount: Mount<'ns, Attached>,
        setup: CommandSetup,
    ) -> Self {
        Self {
            channel,
            mount,
            setup,
        }
    }
    pub fn execute(
        self,
        mut namespace_entry: Command,
        cutoff: SetupCutoff,
    ) -> Result<ReceiverCompletion<'ns>, Box<ReceiverFailure<'ns>>> {
        let setup_deadline = match cutoff.deadline() {
            Ok(deadline) => deadline,
            Err(source) => {
                return Err(Box::new(ReceiverFailure::Before {
                    source,
                    mount: self.mount,
                }));
            }
        };
        let (gate, endpoint) = match ExecutionGate::pair() {
            Ok(pair) => pair,
            Err(source) => {
                return Err(Box::new(ReceiverFailure::Before {
                    source,
                    mount: self.mount,
                }));
            }
        };
        let input = match self.setup.with_gate(endpoint).into_input() {
            Ok(input) => input,
            Err(source) => {
                return Err(Box::new(ReceiverFailure::Before {
                    source,
                    mount: self.mount,
                }));
            }
        };
        namespace_entry
            .env_clear()
            .stdin(input)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut running = self
            .mount
            .launch(namespace_entry)
            .map_err(|error| Box::new(ReceiverFailure::Launch(error)))?;
        let diagnostics = match running.relay_diagnostics() {
            Ok(diagnostics) => diagnostics,
            Err(source) => {
                return Err(Box::new(ReceiverFailure::Startup {
                    source,
                    mount: running,
                }));
            }
        };
        let startup = ReceiverStartup {
            mount: running,
            diagnostics,
            deadline: setup_deadline,
        };
        // Channels remain outside consuming admission operations until those
        // operations need them. In particular a start-gate failure must not
        // close an admitted supervisor channel before bounded local cleanup.
        let gate = match gate.wait_ready_until(setup_deadline) {
            Ok(gate) => gate,
            Err(source) => return Err(startup.failure(source)),
        };
        if let Err(source) = self.channel.report_prepared_until(setup_deadline) {
            drop(gate);
            return Err(startup.failure(source));
        }
        let channel = match self.channel.await_admission_until(setup_deadline) {
            Ok(channel) => channel,
            Err(source) => {
                drop(gate);
                return Err(startup.failure(source));
            }
        };
        if let Err(source) = gate.start_until(setup_deadline) {
            // `channel` stays alive while failure() reaps/drains/closes the view.
            return Err(startup.failure(source));
        }
        startup.finish(channel)
    }
}

/// The receiver's launched namespace child and its diagnostic readers. Failure
/// consumes this owner under the original setup deadline; success waits under
/// the supervisor's independent recipe deadline. The caller retains its control
/// channel while local failure cleanup runs whenever the protocol permits it.
#[derive(Debug)]
struct ReceiverStartup<'ns> {
    mount: Mount<'ns, InUse>,
    diagnostics: crate::process::DiagnosticRelay,
    deadline: Instant,
}
impl<'ns> ReceiverStartup<'ns> {
    fn failure(self, source: io::Error) -> Box<ReceiverFailure<'ns>> {
        let (mount, _) = match self.mount.wait_until(self.deadline) {
            Ok(completion) => completion,
            Err(failure) => {
                return Box::new(ReceiverFailure::Startup {
                    source,
                    mount: failure.into_parts().1,
                });
            }
        };
        if let Err(error) = self.diagnostics.finish() {
            eprintln!("startup diagnostic relay: {error}");
        }
        eprintln!("restricted command startup failed: {source}");
        Box::new(match mount.close() {
            Ok(()) => ReceiverFailure::Cleaned(source),
            Err(cleanup) => ReceiverFailure::Cleanup { source, cleanup },
        })
    }

    fn finish(
        self,
        channel: Supervision<Admitted>,
    ) -> Result<ReceiverCompletion<'ns>, Box<ReceiverFailure<'ns>>> {
        let (mount, status) = self
            .mount
            .wait_under_supervisor()
            .map_err(|error| Box::new(ReceiverFailure::Wait(error)))?;
        if let Err(source) = self.diagnostics.finish() {
            return Err(Box::new(ReceiverFailure::Diagnostics { source, mount }));
        }
        Ok(ReceiverCompletion {
            channel,
            mount,
            status,
        })
    }
}
