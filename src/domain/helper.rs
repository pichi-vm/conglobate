// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CommandDomain, DomainCancellation, DomainJoin};
use crate::process::{Identified, Joined, LaunchFailure, Paired, Process, Supervision, WaitError};
use std::{
    io,
    process::{Command, ExitStatus},
    time::Instant,
};

/// Owns a contained trusted helper, supervision channel and cgroup.
/// This does not configure a command sandbox or authorize recipe execution.
///
#[derive(Debug)]
pub(crate) struct ContainedHelper {
    domain: CommandDomain,
    process: Process<Identified>,
    channel: Supervision<Joined>,
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum HelperLaunchFailure {
    #[error("contained helper launch: {source}")]
    Launch {
        source: LaunchFailure,
        domain: DomainCancellation,
    },
    #[error("contained helper cgroup join: {source}")]
    Join {
        source: io::Error,
        process: Process<Identified>,
        domain: DomainCancellation,
    },
}
impl HelperLaunchFailure {
    pub(crate) fn cancel(self, deadline: Instant) -> (io::Error, anyhow::Result<()>) {
        let (source, domain, terminated) = match self {
            Self::Launch {
                source: LaunchFailure::Before(source),
                domain,
            } => (source, domain, Ok(())),
            Self::Launch {
                source: LaunchFailure::Started { source, process },
                domain,
            } => {
                let result = process
                    .terminate_until(deadline)
                    .map(|_| ())
                    .map_err(|failure| {
                        let (error, process) = failure.into_parts();
                        std::mem::forget(process);
                        anyhow::Error::new(error)
                    });
                (source, domain, result)
            }
            Self::Join {
                source,
                process,
                domain,
            } => {
                let result = process
                    .terminate_until(deadline)
                    .map(|_| ())
                    .map_err(|failure| {
                        let (error, process) = failure.into_parts();
                        std::mem::forget(process);
                        anyhow::Error::new(error)
                    });
                (source, domain, result)
            }
        };
        // Always request cgroup cancellation, even when direct-child reaping failed.
        let cancelled = domain.finish_until(deadline);
        let cleanup = match (terminated, cancelled) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(cleanup), Ok(())) => Err(cleanup.context("helper cleanup")),
            (Ok(()), Err(cleanup)) => Err(anyhow::Error::new(cleanup).context("domain cleanup")),
            (Err(child), Err(domain)) => Err(child.context(format!("domain cleanup: {domain}"))),
        };
        (source, cleanup)
    }
}
impl CommandDomain {
    /// Trusted helper only. Startup errors retain all created resources and
    /// restrict domain ownership to cancellation. The caller must explicitly
    /// terminate/reap any started child and cancel its possibly populated domain.
    pub(crate) fn launch(
        self,
        command: Command,
        deadline: Instant,
    ) -> Result<ContainedHelper, Box<HelperLaunchFailure>> {
        self.finish_launch(
            Process::launch_supervised(command, deadline),
            |channel, permit| channel.join_helper_until(permit, deadline),
        )
    }
    fn finish_launch(
        self,
        launched: Result<(Process<Identified>, Supervision<Paired>), LaunchFailure>,
        join: impl FnOnce(Supervision<Paired>, DomainJoin<'_>) -> io::Result<Supervision<Joined>>,
    ) -> Result<ContainedHelper, Box<HelperLaunchFailure>> {
        let (process, channel) = match launched {
            Ok(pair) => pair,
            Err(source) => {
                return Err(Box::new(HelperLaunchFailure::Launch {
                    source,
                    domain: self.cancellation(),
                }));
            }
        };
        let joined = self.join_permit().and_then(|permit| join(channel, permit));
        match joined {
            Ok(channel) => Ok(ContainedHelper {
                domain: self,
                process,
                channel,
            }),
            Err(source) => Err(Box::new(HelperLaunchFailure::Join {
                source,
                process,
                domain: self.cancellation(),
            })),
        }
    }
}
impl ContainedHelper {
    pub(crate) fn supervision(&self) -> &Supervision<Joined> {
        &self.channel
    }

    /// Close admission, cancel the domain, reap the helper, then drain adoptees.
    /// Any error is terminal. Unresolved process ownership is retained for VM
    /// disposal, and the enclosing Step keeps its namespace and storage.
    pub(crate) fn finish_until(
        self,
        reaper: crate::process::ChildReaper,
        deadline: Instant,
    ) -> io::Result<(ExitStatus, crate::process::ReapedChildren)> {
        let Self {
            domain,
            process,
            channel,
        } = self;
        drop(channel);
        let process = std::mem::ManuallyDrop::new(process);
        domain.cancellation().finish_until(deadline)?;
        let process = std::mem::ManuallyDrop::into_inner(process)
            .wait_until(deadline)
            .map_err(|failure| {
                let (error, process) = failure.into_parts();
                std::mem::forget(process);
                match error {
                    WaitError::Io(error) => error,
                    WaitError::Deadline => {
                        io::Error::new(io::ErrorKind::TimedOut, "helper reap deadline expired")
                    }
                }
            })?;
        let children = reaper
            .drain_until(deadline)
            .map_err(|failure| failure.into_parts().0)?;
        Ok((process.status(), children))
    }
}
