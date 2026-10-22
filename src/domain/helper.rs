// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{DomainCancellation, DomainRecordCleanup, DomainRegistration, RecordedDomain};
use crate::process::{
    Admitted, Identified, LaunchFailure, ObservedState, Paired, Process, Registered, Supervision,
    WaitError,
};
use std::{
    io,
    path::Path,
    process::{Command, ExitStatus},
    time::Instant,
};

/// Owns a registered trusted helper, supervision channel and recorded domain.
/// This does not configure a command sandbox or authorize recipe execution.
///
/// ```compile_fail
/// use conglobate::{domain::ContainedHelper, process::Admitted};
/// fn cannot_admit_twice(helper: ContainedHelper<'_, Admitted>) {
///     helper.admit_until(std::time::Instant::now());
/// }
/// ```
#[derive(Debug)]
pub struct ContainedHelper<'lease, S: ObservedState = Registered> {
    domain: RecordedDomain<'lease>,
    process: Process<Identified>,
    channel: Supervision<S>,
}
#[derive(Debug, thiserror::Error)]
pub enum HelperLaunchFailure<'lease> {
    #[error("contained helper launch: {source}")]
    Launch {
        source: LaunchFailure,
        domain: DomainCancellation<'lease>,
    },
    #[error("contained helper registration: {source}")]
    Registration {
        source: io::Error,
        process: Process<Identified>,
        domain: DomainCancellation<'lease>,
    },
}
/// Admission is closed. Retains both the domain and helper wait owner for retry.
#[derive(Debug)]
pub struct HelperCancellation<'lease> {
    domain: DomainCancellation<'lease>,
    process: Process<Identified>,
}
#[derive(Debug, thiserror::Error)]
#[error("contained helper domain cancellation: {source}")]
pub struct HelperCancelFailure<'lease> {
    source: io::Error,
    cancellation: HelperCancellation<'lease>,
}
impl<'lease> HelperCancelFailure<'lease> {
    pub fn into_parts(self) -> (io::Error, HelperCancellation<'lease>) {
        (self.source, self.cancellation)
    }
}
/// The domain was removed, but its helper status has not been collected.
///
/// ```compile_fail
/// use conglobate::domain::HelperReaping;
/// use std::path::Path;
/// fn cannot_remove_record_before_reaping(owner: HelperReaping<'_>) {
///     owner.finish(Path::new("/unused"));
/// }
/// ```
#[derive(Debug)]
pub struct HelperReaping<'lease> {
    process: Process<Identified>,
    cleanup: DomainRecordCleanup<'lease>,
}
#[derive(Debug, thiserror::Error)]
#[error("contained helper reaping: {source}")]
pub struct HelperReapFailure<'lease> {
    source: WaitError,
    reaping: HelperReaping<'lease>,
}
impl<'lease> HelperReapFailure<'lease> {
    pub fn into_parts(self) -> (WaitError, HelperReaping<'lease>) {
        (self.source, self.reaping)
    }
}
/// The registered helper was reaped after domain cancellation. Its record still
/// needs cleanup. This is not proof that every adopted descendant was reaped.
#[derive(Debug)]
pub struct ReapedHelper<'lease> {
    status: ExitStatus,
    cleanup: DomainRecordCleanup<'lease>,
}
#[derive(Debug, thiserror::Error)]
#[error("contained helper record cleanup: {source}")]
pub struct HelperFinishFailure<'lease> {
    source: io::Error,
    helper: ReapedHelper<'lease>,
}
impl<'lease> HelperFinishFailure<'lease> {
    pub fn into_parts(self) -> (io::Error, ReapedHelper<'lease>) {
        (self.source, self.helper)
    }
}
impl<'lease> RecordedDomain<'lease> {
    /// Trusted helper only. Startup errors retain all created resources and
    /// restrict domain ownership to cancellation. The caller must explicitly
    /// terminate/reap any started child and cancel its possibly populated domain.
    pub fn launch(
        self,
        command: Command,
        deadline: Instant,
    ) -> Result<ContainedHelper<'lease>, Box<HelperLaunchFailure<'lease>>> {
        self.finish_launch(
            Process::launch_supervised(command, deadline),
            |channel, permit| channel.register_helper_until(permit, deadline),
        )
    }
    /// Guardian startup: observe upstream loss during both identity exchange and
    /// registration. Failures retain any started child and a cleanup-only domain.
    /// This does not make kernel spawn interruptible or authorize recipe code.
    pub fn launch_watched(
        self,
        command: Command,
        supervisor: &Supervision<Paired>,
        deadline: Instant,
    ) -> Result<ContainedHelper<'lease>, Box<HelperLaunchFailure<'lease>>> {
        self.finish_launch(
            Process::launch_watched(command, supervisor, deadline),
            |channel, permit| channel.register_helper_watched(permit, supervisor, deadline),
        )
    }
    fn finish_launch(
        self,
        launched: Result<(Process<Identified>, Supervision<Paired>), LaunchFailure>,
        register: impl FnOnce(
            Supervision<Paired>,
            DomainRegistration<'_>,
        ) -> io::Result<Supervision<Registered>>,
    ) -> Result<ContainedHelper<'lease>, Box<HelperLaunchFailure<'lease>>> {
        let (process, channel) = match launched {
            Ok(pair) => pair,
            Err(source) => {
                return Err(Box::new(HelperLaunchFailure::Launch {
                    source,
                    domain: self.cancellation(),
                }));
            }
        };
        let registered = self
            .registration()
            .and_then(|permit| register(channel, permit));
        match registered {
            Ok(channel) => Ok(ContainedHelper {
                domain: self,
                process,
                channel,
            }),
            Err(source) => Err(Box::new(HelperLaunchFailure::Registration {
                source,
                process,
                domain: self.cancellation(),
            })),
        }
    }
}
impl<'lease, S: ObservedState> ContainedHelper<'lease, S> {
    pub fn supervision(&self) -> &Supervision<S> {
        &self.channel
    }
    /// Consumes registration authority and closes the channel before requesting
    /// subtree cancellation. Failures retain cleanup-only ownership.
    pub fn cancel_until(
        self,
        deadline: Instant,
    ) -> Result<HelperReaping<'lease>, Box<HelperCancelFailure<'lease>>> {
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
impl<'lease> HelperCancellation<'lease> {
    pub fn retry_until(
        self,
        deadline: Instant,
    ) -> Result<HelperReaping<'lease>, Box<HelperCancelFailure<'lease>>> {
        let Self { domain, process } = self;
        match domain.retry_until(deadline) {
            Ok(cleanup) => Ok(HelperReaping { process, cleanup }),
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
impl<'lease> HelperReaping<'lease> {
    pub fn reap_until(
        self,
        deadline: Instant,
    ) -> Result<ReapedHelper<'lease>, Box<HelperReapFailure<'lease>>> {
        let Self { process, cleanup } = self;
        match process.wait_until(deadline) {
            Ok(process) => Ok(ReapedHelper {
                status: process.status(),
                cleanup,
            }),
            Err(failure) => {
                let (source, process) = failure.into_parts();
                Err(Box::new(HelperReapFailure {
                    source,
                    reaping: Self { process, cleanup },
                }))
            }
        }
    }
}
impl<'lease> ReapedHelper<'lease> {
    /// Return the helper's diagnostic exit status only after record cleanup.
    /// No policy/filesystem completion or successful image receipt is produced.
    pub fn finish(self, parent: &Path) -> Result<ExitStatus, HelperFinishFailure<'lease>> {
        let Self { status, cleanup } = self;
        match cleanup.finish(parent) {
            Ok(()) => Ok(status),
            Err(failure) => {
                let (source, cleanup) = failure.into_parts();
                Err(HelperFinishFailure {
                    source,
                    helper: Self { status, cleanup },
                })
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("command admission handshake: {source}")]
pub struct AdmissionFailure<'lease> {
    source: io::Error,
    cancellation: HelperCancellation<'lease>,
}
impl<'lease> AdmissionFailure<'lease> {
    pub fn into_parts(self) -> (io::Error, HelperCancellation<'lease>) {
        (self.source, self.cancellation)
    }
}
impl<'lease> ContainedHelper<'lease, Registered> {
    /// Release a registered trusted helper. The enclosing step must establish
    /// policy and isolation prerequisites before calling; registration alone
    /// never authorizes image commands. Failure permits cancellation only.
    /// A failed acknowledgment may follow successful delivery: never assume
    /// admission failure proves that image execution did not start.
    pub fn admit_until(
        self,
        deadline: Instant,
    ) -> Result<ContainedHelper<'lease, Admitted>, Box<AdmissionFailure<'lease>>> {
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
