// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{CommandDomain, DomainCancellation, DomainJoin};
use crate::process::{
    Admitted, Identified, Joined, LaunchFailure, ObservedState, Paired, Process, Supervision,
    WaitError,
};
use std::{
    io,
    process::{Command, ExitStatus},
    time::Instant,
};

/// Owns a contained trusted helper, supervision channel and cgroup.
/// This does not configure a command sandbox or authorize recipe execution.
///
#[derive(Debug)]
pub(crate) struct ContainedHelper<S: ObservedState = Joined> {
    domain: CommandDomain,
    process: Process<Identified>,
    channel: Supervision<S>,
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
        let cancelled = domain.retry_until(deadline).map_err(|failure| {
            let (error, domain) = failure.into_parts();
            std::mem::forget(domain);
            error
        });
        let cleanup = match (terminated, cancelled) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(cleanup), Ok(())) => Err(cleanup.context("helper cleanup")),
            (Ok(()), Err(cleanup)) => Err(anyhow::Error::new(cleanup).context("domain cleanup")),
            (Err(child), Err(domain)) => Err(child.context(format!("domain cleanup: {domain}"))),
        };
        (source, cleanup)
    }
}
/// Admission is closed. Retains the domain and helper for bounded cancellation.
#[derive(Debug)]
pub(crate) struct HelperCancellation {
    domain: DomainCancellation,
    process: Process<Identified>,
}
#[derive(Debug, thiserror::Error)]
#[error("contained helper domain cancellation: {source}")]
pub(crate) struct HelperCancelFailure {
    source: io::Error,
    cancellation: HelperCancellation,
}
impl HelperCancelFailure {
    pub(crate) fn into_parts(self) -> (io::Error, HelperCancellation) {
        (self.source, self.cancellation)
    }
}
/// The domain was removed, but its helper status has not been collected.
///
#[derive(Debug)]
pub(crate) struct HelperReaping {
    process: Process<Identified>,
}
#[derive(Debug, thiserror::Error)]
#[error("contained helper reaping: {source}")]
pub(crate) struct HelperReapFailure {
    source: WaitError,
    reaping: HelperReaping,
}
impl HelperReapFailure {
    pub(crate) fn into_parts(self) -> (WaitError, HelperReaping) {
        (self.source, self.reaping)
    }
}
/// The joined helper was reaped after domain cancellation. Adopted descendants
/// still require a separate drain.
#[derive(Debug)]
pub(crate) struct ReapedHelper {
    status: ExitStatus,
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
impl<S: ObservedState> ContainedHelper<S> {
    pub(crate) fn supervision(&self) -> &Supervision<S> {
        &self.channel
    }
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
        HelperCancellation {
            domain: domain.cancellation(),
            process,
        }
        .finish_until(reaper, deadline)
    }
    /// Consumes domain-join authority and closes the channel before requesting
    /// subtree cancellation. Failures retain cleanup-only ownership.
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn cancel_until(
        self,
        deadline: Instant,
    ) -> Result<HelperReaping, Box<HelperCancelFailure>> {
        let Self {
            domain,
            process,
            channel,
        } = self;
        drop(channel);
        HelperCancellation {
            domain: domain.cancellation(),
            process,
        }
        .retry_until(deadline)
    }
}
impl HelperCancellation {
    pub(crate) fn finish_until(
        self,
        reaper: crate::process::ChildReaper,
        deadline: Instant,
    ) -> io::Result<(ExitStatus, crate::process::ReapedChildren)> {
        let helper = self.retry_until(deadline).map_err(|failure| {
            let (error, owner) = failure.into_parts();
            std::mem::forget(owner);
            error
        })?;
        let helper = helper.reap_until(deadline).map_err(|failure| {
            let (error, owner) = failure.into_parts();
            std::mem::forget(owner);
            match error {
                WaitError::Io(error) => error,
                WaitError::Deadline => {
                    io::Error::new(io::ErrorKind::TimedOut, "helper reap deadline expired")
                }
            }
        })?;
        let done = helper
            .with_reaper(reaper)
            .drain_until(deadline)
            .map_err(|failure| {
                let (error, _) = failure.into_parts();
                error
            })?;
        Ok(done.finish())
    }

    pub(crate) fn retry_until(
        self,
        deadline: Instant,
    ) -> Result<HelperReaping, Box<HelperCancelFailure>> {
        let Self { domain, process } = self;
        match domain.retry_until(deadline) {
            Ok(()) => Ok(HelperReaping { process }),
            Err(failure) => {
                let (source, domain) = failure.into_parts();
                Err(Box::new(HelperCancelFailure {
                    source,
                    cancellation: Self { domain, process },
                }))
            }
        }
    }
}
impl HelperReaping {
    pub(crate) fn reap_until(
        self,
        deadline: Instant,
    ) -> Result<ReapedHelper, Box<HelperReapFailure>> {
        let Self { process } = self;
        match process.wait_until(deadline) {
            Ok(process) => Ok(ReapedHelper {
                status: process.status(),
            }),
            Err(failure) => {
                let (source, process) = failure.into_parts();
                Err(Box::new(HelperReapFailure {
                    source,
                    reaping: Self { process },
                }))
            }
        }
    }
}
impl ReapedHelper {
    pub(crate) fn finish(self) -> ExitStatus {
        self.status
    }
}

#[derive(Debug, thiserror::Error)]
#[error("command admission handshake: {source}")]
pub(crate) struct AdmissionFailure {
    source: io::Error,
    cancellation: HelperCancellation,
}
impl AdmissionFailure {
    pub(crate) fn into_parts(self) -> (io::Error, HelperCancellation) {
        (self.source, self.cancellation)
    }
}
impl ContainedHelper<Joined> {
    /// Release a contained trusted helper. The enclosing step must establish
    /// policy and isolation prerequisites before calling; cgroup membership alone
    /// never authorizes image commands. Failure permits cancellation only.
    /// A failed acknowledgment may follow successful delivery: never assume
    /// admission failure proves that image execution did not start.
    pub(crate) fn admit_until(
        self,
        deadline: Instant,
    ) -> Result<ContainedHelper<Admitted>, Box<AdmissionFailure>> {
        let Self {
            domain,
            process,
            channel,
        } = self;
        match channel.admit_until(deadline) {
            Ok(channel) => Ok(ContainedHelper {
                domain,
                process,
                channel,
            }),
            Err(source) => Err(Box::new(AdmissionFailure {
                source,
                cancellation: HelperCancellation {
                    domain: domain.cancellation(),
                    process,
                },
            })),
        }
    }
}
