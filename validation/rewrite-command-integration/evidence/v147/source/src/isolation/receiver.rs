// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{CommandSetup, ExecutionGate};
use crate::{
    filesystem::{Attached, InUse, Mount, RootLaunchFailure, RootWaitFailure},
    process::{Admitted, Registered, Supervision},
    step::SetupCutoff,
};
use std::{
    io,
    process::{Command, ExitStatus},
};

/// Outer trusted helper, already contained and in its private mount namespace.
/// The supervisor retains the volume, policy and cancellation authority. Errors
/// are exit-only in this helper; returned resources remain owned for teardown.
#[derive(Debug)]
pub struct CommandReceiver<'ns> {
    channel: Supervision<Registered>,
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
        channel: Supervision<Registered>,
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
        namespace_entry.env_clear().stdin(input);
        let running = self
            .mount
            .launch(namespace_entry)
            .map_err(|error| Box::new(ReceiverFailure::Launch(error)))?;
        let admitted = (|| -> io::Result<Supervision<Admitted>> {
            let gate = gate.wait_ready_until(setup_deadline)?;
            self.channel.report_prepared_until(setup_deadline)?;
            let channel = self.channel.await_admission_until(setup_deadline)?;
            gate.start_until(setup_deadline)?;
            Ok(channel)
        })();
        let channel = match admitted {
            Ok(channel) => channel,
            Err(source) => {
                return Err(Box::new(ReceiverFailure::Startup {
                    source,
                    mount: running,
                }));
            }
        };
        let (mount, status) = running
            .wait_under_supervisor()
            .map_err(|error| Box::new(ReceiverFailure::Wait(error)))?;
        Ok(ReceiverCompletion {
            channel,
            mount,
            status,
        })
    }
}
