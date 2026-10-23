// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Ownership across acquisition and helper launch, before the ready Step exists.
mod sandbox;
mod storage;
use super::{Ready, Retained, Step, teardown::Deadline};
use crate::{
    domain::{
        CommandDomain, DomainCancellation, DomainCreateFailure, DomainCreationRollback,
        HelperLaunchFailure,
    },
    isolation::{Mapped, UserNamespace},
    process::{ChildReaper, Identified, LaunchFailure, Process, Running},
    storage::{BootstrapVolumes, FailureDiagnostics, Mounted, Volume},
};
pub use sandbox::{CommandSandbox, SandboxDirectories};
use std::{
    io,
    mem::ManuallyDrop,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};
pub use storage::{PreparationStorage, StorageLayout};

#[derive(Debug)]
struct Assigned<R: PreparationStorage> {
    namespace: ManuallyDrop<UserNamespace<Mapped>>,
    volume: Retained<R>,
}
/// Namespace ownership and mounted storage stay together even if
/// helper startup fails before cgroup joining. The reaper must already be installed.
#[derive(Debug)]
pub struct StoragePreparation<R: PreparationStorage> {
    namespace: ManuallyDrop<UserNamespace<Mapped>>,
    volume: Retained<R>,
    domain: CommandDomain,
    reaper: ChildReaper,
}
impl<R: PreparationStorage> StoragePreparation<R> {
    /// Acquire and publish containment before launching a helper. Every failure
    /// retains the supplied namespace, mounted storage and child reaper.
    pub fn create(
        namespace: UserNamespace<Mapped>,
        volume: R,
        reaper: ChildReaper,
        recovery: &crate::anchor::RecoveryDirectory,
        domain_parent: &Path,
        budget: &super::SetupBudget,
    ) -> Result<Self, Box<StoragePreparationFailure<R>>> {
        let domain = budget
            .check()
            .map_err(DomainCreateFailure::Before)
            .and_then(|()| CommandDomain::create(recovery, domain_parent));
        match domain {
            Ok(domain) => Ok(Self::new(namespace, volume, domain, reaper)),
            Err(failure) => {
                let (source, rollback) = failure.into_rollback();
                Err(Box::new(StoragePreparationFailure {
                    diagnostics: source.into(),
                    phase: Phase::DomainSetup(
                        rollback,
                        reaper,
                        Assigned {
                            namespace: ManuallyDrop::new(namespace),
                            volume: Retained::new(volume),
                        },
                    ),
                    deadline: Instant::now() + Duration::from_secs(30),
                }))
            }
        }
    }

    pub fn new(
        namespace: UserNamespace<Mapped>,
        volume: R,
        domain: CommandDomain,
        reaper: ChildReaper,
    ) -> Self {
        Self {
            namespace: ManuallyDrop::new(namespace),
            volume: Retained::new(volume),
            domain,
            reaper,
        }
    }
    pub fn launch<'l>(
        self,
        command: Command,
        setup_deadline: Instant,
    ) -> Result<Step<'l, Ready, R>, Box<StoragePreparationFailure<R>>> {
        let Self {
            namespace,
            volume,
            domain,
            reaper,
        } = self;
        match domain.launch(command, setup_deadline) {
            Ok(helper) => Ok(
                Step::new(ManuallyDrop::into_inner(namespace), helper, reaper)
                    .with_storage(volume.release()),
            ),
            Err(failure) => {
                let assigned = Assigned { namespace, volume };
                let (source, phase) = match *failure {
                    HelperLaunchFailure::Launch {
                        source: LaunchFailure::Before(source),
                        domain,
                    } => (source, Phase::Domain(domain, reaper, assigned)),
                    HelperLaunchFailure::Launch {
                        source: LaunchFailure::Started { source, process },
                        domain,
                    } => (source, Phase::Running(process, domain, reaper, assigned)),
                    HelperLaunchFailure::Join {
                        source,
                        process,
                        domain,
                    } => (source, Phase::Identified(process, domain, reaper, assigned)),
                };
                Err(Box::new(StoragePreparationFailure {
                    diagnostics: source.into(),
                    phase,
                    deadline: Instant::now() + Duration::from_secs(30),
                }))
            }
        }
    }
}
#[derive(Debug)]
enum Phase<R: PreparationStorage> {
    MountDrain(ChildReaper, R::MountFailure),
    MountRollback(R::MountFailure),
    EarlyRunning(Process<Running>, ChildReaper, R::Unmounted),
    EarlyIdentified(Process<Identified>, ChildReaper, R::Unmounted),
    EarlyDrain(ChildReaper, R::Unmounted),

    DomainSetup(DomainCreationRollback, ChildReaper, Assigned<R>),
    EmptyDrain(ChildReaper, Assigned<R>),
    Running(
        Process<Running>,
        DomainCancellation,
        ChildReaper,
        Assigned<R>,
    ),
    Identified(
        Process<Identified>,
        DomainCancellation,
        ChildReaper,
        Assigned<R>,
    ),
    Domain(DomainCancellation, ChildReaper, Assigned<R>),
    Unmount(Retained<R>),
    UnmountRetry(R::UnmountFailure),
    Complete(R::Unmounted),
}
/// Cleanup-only owner. No retry can restart launch or renew its fixed budget.
/// Dropping before quiescence retains namespace and storage for VM disposal.
/// ```compile_fail
/// use conglobate::step::PreparationFailure;
/// fn cannot_extract_live_storage(failure: PreparationFailure<'_, '_, '_>) {
///     failure.into_parts();
/// }
/// ```
/// ```compile_fail
/// use conglobate::step::PreparationFailure;
/// fn cannot_renew_cleanup_budget(failure: PreparationFailure<'_, '_, '_>) {
///     failure.rollback(std::time::Instant::now());
/// }
/// ```
#[derive(Debug, thiserror::Error)]
#[error("command preparation: {diagnostics}")]
pub struct StoragePreparationFailure<R: PreparationStorage> {
    diagnostics: FailureDiagnostics,
    phase: Phase<R>,
    deadline: Instant,
}
/// Failed setup was completely rolled back. This is never a successful step.
#[derive(Debug)]
pub struct StorageRollback<R: PreparationStorage> {
    diagnostics: FailureDiagnostics,
    volume: R::Unmounted,
}
impl<R: PreparationStorage> StorageRollback<R> {
    pub fn into_parts(self) -> (FailureDiagnostics, R::Unmounted) {
        (self.diagnostics, self.volume)
    }
}
impl<R: PreparationStorage> StoragePreparationFailure<R> {
    fn early(source: impl Into<FailureDiagnostics>, phase: Phase<R>) -> Box<Self> {
        Box::new(Self {
            diagnostics: source.into(),
            phase,
            deadline: Instant::now() + Duration::from_secs(30),
        })
    }

    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn rollback(self) -> Result<StorageRollback<R>, Box<Self>> {
        if let Err(error) = Deadline::check(self.deadline) {
            return Err(Box::new(Self {
                diagnostics: self.diagnostics.followed_by(error.into()),
                ..self
            }));
        }
        let Self {
            diagnostics,
            phase,
            deadline,
        } = self;
        let next: Result<Phase<R>, (FailureDiagnostics, Phase<R>)> = match phase {
            Phase::MountDrain(reaper, failure) => match reaper.drain_until(deadline) {
                Ok(_) => Ok(Phase::MountRollback(failure)),
                Err(error) => {
                    let (source, reaper) = error.into_parts();
                    Err((source.into(), Phase::MountDrain(reaper, failure)))
                }
            },
            Phase::MountRollback(failure) => match R::rollback_mount(failure, deadline) {
                Ok(volume) => Ok(Phase::Complete(volume)),
                Err(failure) => Err((
                    R::unmount_diagnostics(&failure).latest_only(),
                    Phase::UnmountRetry(failure),
                )),
            },
            Phase::EarlyRunning(process, reaper, volume) => {
                match process.terminate_until(deadline) {
                    Ok(_) => Ok(Phase::EarlyDrain(reaper, volume)),
                    Err(failure) => {
                        let (source, process) = failure.into_parts();
                        Err((
                            io::Error::other(source).into(),
                            Phase::EarlyRunning(process, reaper, volume),
                        ))
                    }
                }
            }
            Phase::EarlyIdentified(process, reaper, volume) => {
                match process.terminate_until(deadline) {
                    Ok(_) => Ok(Phase::EarlyDrain(reaper, volume)),
                    Err(failure) => {
                        let (source, process) = failure.into_parts();
                        Err((
                            io::Error::other(source).into(),
                            Phase::EarlyIdentified(process, reaper, volume),
                        ))
                    }
                }
            }
            Phase::EarlyDrain(reaper, volume) => match reaper.drain_until(deadline) {
                Ok(_) => Ok(Phase::Complete(volume)),
                Err(failure) => {
                    let (source, reaper) = failure.into_parts();
                    Err((source.into(), Phase::EarlyDrain(reaper, volume)))
                }
            },
            Phase::DomainSetup(rollback, reaper, assigned) => {
                match rollback.retry_until(deadline) {
                    Ok(()) => Ok(Phase::EmptyDrain(reaper, assigned)),
                    Err(failure) => {
                        let (error, rollback) = failure.into_parts();
                        Err((error.into(), Phase::DomainSetup(rollback, reaper, assigned)))
                    }
                }
            }
            Phase::EmptyDrain(reaper, assigned) => match reaper.drain_until(deadline) {
                Ok(_) => {
                    drop(ManuallyDrop::into_inner(assigned.namespace));
                    Ok(Phase::Unmount(assigned.volume))
                }
                Err(failure) => {
                    let (error, reaper) = failure.into_parts();
                    Err((error.into(), Phase::EmptyDrain(reaper, assigned)))
                }
            },
            Phase::Running(process, domain, reaper, assigned) => {
                match process.terminate_until(deadline) {
                    Ok(_) => Ok(Phase::Domain(domain, reaper, assigned)),
                    Err(failure) => {
                        let (error, process) = failure.into_parts();
                        Err((
                            io::Error::other(error).into(),
                            Phase::Running(process, domain, reaper, assigned),
                        ))
                    }
                }
            }
            Phase::Identified(process, domain, reaper, assigned) => {
                match process.terminate_until(deadline) {
                    Ok(_) => Ok(Phase::Domain(domain, reaper, assigned)),
                    Err(failure) => {
                        let (error, process) = failure.into_parts();
                        Err((
                            io::Error::other(error).into(),
                            Phase::Identified(process, domain, reaper, assigned),
                        ))
                    }
                }
            }
            Phase::Domain(domain, reaper, assigned) => match domain.retry_until(deadline) {
                Ok(()) => Ok(Phase::EmptyDrain(reaper, assigned)),
                Err(failure) => {
                    let (error, domain) = failure.into_parts();
                    Err((error.into(), Phase::Domain(domain, reaper, assigned)))
                }
            },
            Phase::Unmount(volume) => match volume.release().unmount_until(deadline) {
                Ok(volume) => Ok(Phase::Complete(volume)),
                Err(failure) => Err((
                    R::unmount_diagnostics(&failure).latest_only(),
                    Phase::UnmountRetry(failure),
                )),
            },
            Phase::UnmountRetry(failure) => match R::retry_unmount(failure) {
                Ok(volume) => Ok(Phase::Complete(volume)),
                Err(failure) => Err((
                    R::unmount_diagnostics(&failure).latest_only(),
                    Phase::UnmountRetry(failure),
                )),
            },
            Phase::Complete(volume) => {
                return Ok(StorageRollback {
                    diagnostics,
                    volume,
                });
            }
        };
        match next {
            Ok(phase) => Self {
                diagnostics,
                phase,
                deadline,
            }
            .rollback(),
            Err((error, phase)) => Err(Box::new(Self {
                diagnostics: diagnostics.followed_by(error),
                phase,
                deadline,
            })),
        }
    }
}

/// Single-volume preparation uses the shared storage lifecycle.
pub type CommandPreparation<'n, 'o, 'w> = StoragePreparation<Volume<'n, 'o, 'w, Mounted>>;
pub type PreparationFailure<'n, 'o, 'w> = StoragePreparationFailure<Volume<'n, 'o, 'w, Mounted>>;
/// Bootstrap preparation retains both roots through the same namespace/process rollback.
pub type BootstrapPreparation<'n, 'o, 'w> =
    StoragePreparation<BootstrapVolumes<'n, 'o, 'w, Mounted>>;
pub type BootstrapPreparationFailure<'n, 'o, 'w> =
    StoragePreparationFailure<BootstrapVolumes<'n, 'o, 'w, Mounted>>;
