// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CommandSetup, ExecutionEndpoint};
use crate::{
    filesystem::{Attached, InUse, Mount, RootWaitFailure},
    process::{Joined, Supervision},
    step::SetupCutoff,
};
use std::{
    io,
    process::{Command, ExitStatus, Stdio},
};

/// Outer trusted helper, already contained and in its private mount namespace.
/// The supervisor retains the volume, mapped namespace and cancellation authority. Errors
/// are exit-only in this helper; returned resources remain owned for teardown.
#[derive(Debug)]
pub(crate) struct CommandReceiver<'ns> {
    channel: Supervision<Joined>,
    mount: Mount<'ns, Attached>,
    setup: CommandSetup,
    endpoint: ExecutionEndpoint,
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum ReceiverFailure<'ns> {
    #[error("receiver preparation: {source}")]
    Before {
        source: io::Error,
        mount: Mount<'ns, Attached>,
    },
    #[error("receiver launch: {0}")]
    Launch(io::Error),
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
}
/// Direct command helper reaped. The caller must close this view and finish
/// local namespace cleanup before reporting cleanup completion. Shell status
/// travels directly from init to the supervisor. Descendant quiescence
/// is still the supervisor's responsibility; this is not build success.
#[derive(Debug)]
pub(crate) struct ReceiverCompletion<'ns> {
    channel: Supervision<Joined>,
    mount: Mount<'ns, Attached>,
    status: ExitStatus,
}
impl<'ns> ReceiverCompletion<'ns> {
    pub(crate) fn into_parts(self) -> (Supervision<Joined>, Mount<'ns, Attached>, ExitStatus) {
        (self.channel, self.mount, self.status)
    }
}
impl<'ns> CommandReceiver<'ns> {
    pub(crate) fn new(
        channel: Supervision<Joined>,
        mount: Mount<'ns, Attached>,
        setup: CommandSetup,
        endpoint: ExecutionEndpoint,
    ) -> Self {
        Self {
            channel,
            mount,
            setup,
            endpoint,
        }
    }
    pub(crate) fn execute(
        self,
        mut namespace_entry: Command,
        cutoff: SetupCutoff,
    ) -> Result<ReceiverCompletion<'ns>, Box<ReceiverFailure<'ns>>> {
        if let Err(source) = cutoff.deadline() {
            return Err(Box::new(ReceiverFailure::Before {
                source,
                mount: self.mount,
            }));
        }
        let input = match self.setup.with_gate(self.endpoint).into_input() {
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
        let (mount, status) = running
            .wait_under_supervisor()
            .map_err(|error| Box::new(ReceiverFailure::Wait(error)))?;
        if let Err(source) = diagnostics.finish() {
            return Err(Box::new(ReceiverFailure::Diagnostics { source, mount }));
        }
        Ok(ReceiverCompletion {
            channel: self.channel,
            mount,
            status,
        })
    }
}
