// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{
    ChildLease, Identified, IdentityReceiver, Paired, Pinned, Process, Running, Supervision,
};
use std::{
    io,
    process::{Command, Stdio},
    time::Instant,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum LaunchFailure {
    #[error("helper launch before child creation: {0}")]
    Before(io::Error),
    #[error("helper identity exchange: {source}")]
    Started {
        source: io::Error,
        process: Process<Running>,
    },
}

impl Process<Identified> {
    /// Launch a trusted helper implementing Supervision startup. The returned
    /// channel observes mutual loss; it does not authorize commands. Uses the
    /// same exclusive reaping/SIGCHLD prerequisites and spawn deadline limits as
    /// launch. Exchange failure cancels the channel and retains the child owner.
    pub(crate) fn launch_supervised(
        command: Command,
        deadline: Instant,
    ) -> Result<(Self, Supervision<Paired>), LaunchFailure> {
        Self::launch_supervised_with(command, deadline, &deadline)
    }

    fn launch_supervised_with(
        mut command: Command,
        deadline: Instant,
        wait: &impl super::identity::ExchangeWait,
    ) -> Result<(Self, Supervision<Paired>), LaunchFailure> {
        if Instant::now() >= deadline {
            return Err(LaunchFailure::Before(io::Error::new(
                io::ErrorKind::TimedOut,
                "helper setup deadline expired",
            )));
        }
        let (parent, child_endpoint) = Supervision::pair().map_err(LaunchFailure::Before)?;
        command.stdin(Stdio::from(child_endpoint));
        let child = command.spawn().map_err(LaunchFailure::Before)?;
        drop(command);
        let mut pending: Process<Running> = child.into();
        let channel = match parent.exchange_with(wait) {
            Ok(channel) => channel,
            Err(source) => {
                return Err(LaunchFailure::Started {
                    source,
                    process: pending,
                });
            }
        };
        let observation = match channel.duplicate_observation() {
            Ok(observation) => observation,
            Err(source) => {
                return Err(LaunchFailure::Started {
                    source,
                    process: pending,
                });
            }
        };
        let child = pending.resources.child.take().expect("owned pending child");
        Ok((
            Self {
                resources: ChildLease {
                    child: Some(child),
                    termination: Pinned { observation },
                },
            },
            channel,
        ))
    }

    /// Launch trusted helper code implementing IdentitySender startup. This
    /// replaces stdin with the private identity channel; all other command
    /// configuration belongs to the trusted caller. Never pass recipe commands.
    ///
    /// Child cleanup requires normal SIGCHLD zombie semantics and exclusive
    /// reaping. This operation does not configure those process-wide settings,
    /// establish a command sandbox, register containment or grant admission.
    /// The deadline bounds exchange/acceptance, not the kernel's spawn operation.
    pub(crate) fn launch(mut command: Command, deadline: Instant) -> Result<Self, LaunchFailure> {
        if Instant::now() >= deadline {
            return Err(LaunchFailure::Before(io::Error::new(
                io::ErrorKind::TimedOut,
                "helper setup deadline expired",
            )));
        }
        let (receiver, sender) = IdentityReceiver::pair().map_err(LaunchFailure::Before)?;
        command.stdin(Stdio::from(sender));
        let child = command.spawn().map_err(LaunchFailure::Before)?;
        // Command retains its configured stdio. Close the parent's copy of the
        // child's endpoint so EOF reflects actual helper loss during exchange.
        drop(command);
        let mut pending: Process<Running> = child.into();
        let observation = match receiver.receive_until(deadline) {
            Ok(observation) => observation,
            Err(source) => {
                return Err(LaunchFailure::Started {
                    source,
                    process: pending,
                });
            }
        };
        let child = pending.resources.child.take().expect("owned pending child");
        Ok(Self {
            resources: ChildLease {
                child: Some(child),
                termination: Pinned { observation },
            },
        })
    }

    pub(crate) fn has_exited(&self) -> io::Result<bool> {
        self.resources.termination.observation.has_exited()
    }
}
