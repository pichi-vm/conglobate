// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Ownership across acquisition and helper launch, before the ready Step exists.
mod sandbox;
mod storage;
use super::{Ready, Retained, Step, pins::PinCleanup, teardown::Deadline};
use crate::{
    domain::{
        DomainCancellation, DomainCreateFailure, DomainCreationRollback, DomainRecordCleanup,
        HelperLaunchFailure, RecordDomainFailure, RecordedDomain,
    },
    policy::{BoundCleanup, BoundDisabled, InstallationFailure, LabelPolicy, PolicyInstallation},
    process::{ChildReaper, Identified, LaunchFailure, Process, Running},
    storage::{BootstrapVolumes, FailureDiagnostics, Mounted, Volume},
};
pub use sandbox::{CommandSandbox, SandboxDirectories};
use std::{
    io,
    mem::ManuallyDrop,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};
pub use storage::{PreparationStorage, StorageLayout, StoragePinFailure};

#[derive(Debug)]
enum PolicyAssignment {
    Uninstalled(ManuallyDrop<Box<PolicyInstallation>>),
    Bound(Box<ManuallyDrop<LabelPolicy<BoundDisabled>>>),
    Failed(ManuallyDrop<Box<InstallationFailure>>),
}
#[derive(Debug)]
struct Assigned<R: PreparationStorage> {
    policy: PolicyAssignment,
    volume: Retained<R>,
}
/// Bound policy, pinned identities and mounted storage stay together even if
/// helper startup fails before registration. The reaper must already be installed.
#[derive(Debug)]
pub struct StoragePreparation<'l, R: PreparationStorage> {
    policy: ManuallyDrop<LabelPolicy<BoundDisabled>>,
    volume: Retained<R>,
    domain: RecordedDomain<'l>,
    reaper: ChildReaper,
}
impl<'l, R: PreparationStorage> StoragePreparation<'l, R> {
    /// Acquire and publish containment before installing policy. Every failure
    /// retains the supplied identity pins, mounted storage and child reaper.
    pub fn create(
        installation: PolicyInstallation,
        volume: R,
        reaper: ChildReaper,
        recovery: &'l crate::anchor::RecoveryDirectory,
        domain_parent: &Path,
        budget: &super::SetupBudget,
    ) -> Result<Self, Box<StoragePreparationFailure<'l, R>>> {
        let domain = budget
            .check()
            .map_err(|error| RecordDomainFailure::Create(DomainCreateFailure::Before(error)))
            .and_then(|()| RecordedDomain::create(recovery, domain_parent));
        match domain {
            Ok(domain) => {
                Self::install(installation, volume, domain, reaper, budget, domain_parent)
            }
            Err(failure) => {
                let (source, rollback) = failure.into_rollback(recovery);
                Err(Box::new(StoragePreparationFailure {
                    diagnostics: source.into(),
                    phase: Phase::DomainSetup(
                        rollback,
                        reaper,
                        Assigned {
                            policy: PolicyAssignment::Uninstalled(ManuallyDrop::new(Box::new(
                                installation,
                            ))),
                            volume: Retained::new(volume),
                        },
                    ),
                    parent: domain_parent.to_owned(),
                    deadline: Instant::now() + Duration::from_secs(30),
                }))
            }
        }
    }

    pub fn install(
        installation: PolicyInstallation,
        volume: R,
        domain: RecordedDomain<'l>,
        reaper: ChildReaper,
        budget: &super::SetupBudget,
        domain_parent: &Path,
    ) -> Result<Self, Box<StoragePreparationFailure<'l, R>>> {
        match installation.install(budget.deadline()) {
            Ok(policy) => Ok(Self::new(policy, volume, domain, reaper)),
            Err(failure) => {
                let diagnostics = failure.diagnostics().latest_only();
                Err(Box::new(StoragePreparationFailure {
                    diagnostics,
                    phase: Phase::Domain(
                        domain.cancellation(),
                        reaper,
                        Assigned {
                            policy: PolicyAssignment::Failed(ManuallyDrop::new(failure)),
                            volume: Retained::new(volume),
                        },
                    ),
                    parent: domain_parent.to_owned(),
                    deadline: Instant::now() + Duration::from_secs(30),
                }))
            }
        }
    }

    pub fn new(
        policy: LabelPolicy<BoundDisabled>,
        volume: R,
        domain: RecordedDomain<'l>,
        reaper: ChildReaper,
    ) -> Self {
        Self {
            policy: ManuallyDrop::new(policy),
            volume: Retained::new(volume),
            domain,
            reaper,
        }
    }
    pub fn launch(
        self,
        command: Command,
        setup_deadline: Instant,
        domain_parent: &Path,
    ) -> Result<Step<'l, Ready, R>, Box<StoragePreparationFailure<'l, R>>> {
        let Self {
            policy,
            volume,
            domain,
            reaper,
        } = self;
        match domain.launch(command, setup_deadline) {
            Ok(helper) => Ok(Step::new(ManuallyDrop::into_inner(policy), helper, reaper)
                .with_storage(volume.release())),
            Err(failure) => {
                let assigned = Assigned {
                    policy: PolicyAssignment::Bound(Box::new(policy)),
                    volume,
                };
                let (source, phase) = match *failure {
                    HelperLaunchFailure::Launch {
                        source: LaunchFailure::Before(source),
                        domain,
                    } => (source, Phase::Domain(domain, reaper, assigned)),
                    HelperLaunchFailure::Launch {
                        source: LaunchFailure::Started { source, process },
                        domain,
                    } => (source, Phase::Running(process, domain, reaper, assigned)),
                    HelperLaunchFailure::Registration {
                        source,
                        process,
                        domain,
                    } => (source, Phase::Identified(process, domain, reaper, assigned)),
                };
                Err(Box::new(StoragePreparationFailure {
                    diagnostics: source.into(),
                    phase,
                    parent: domain_parent.to_owned(),
                    deadline: Instant::now() + Duration::from_secs(30),
                }))
            }
        }
    }
}
#[derive(Debug)]
enum Phase<'l, R: PreparationStorage> {
    MountPins(PinCleanup, ChildReaper, R::MountFailure),
    MountDrain(ChildReaper, R::MountFailure),
    MountRollback(R::MountFailure),
    EarlyRunning(Process<Running>, ChildReaper, R::Unmounted),
    EarlyIdentified(Process<Identified>, ChildReaper, R::Unmounted),
    EarlyDrain(ChildReaper, R::Unmounted),
    EarlyPins(PinCleanup, ChildReaper, R::Unmounted),
    MountedPins(PinCleanup, ChildReaper, Retained<R>),
    MountedDrain(ChildReaper, Retained<R>),

    DomainSetup(DomainCreationRollback<'l>, ChildReaper, Assigned<R>),
    EmptyDrain(ChildReaper, Assigned<R>),
    Running(
        Process<Running>,
        DomainCancellation<'l>,
        ChildReaper,
        Assigned<R>,
    ),
    Identified(
        Process<Identified>,
        DomainCancellation<'l>,
        ChildReaper,
        Assigned<R>,
    ),
    Domain(DomainCancellation<'l>, ChildReaper, Assigned<R>),
    Drain(DomainRecordCleanup<'l>, ChildReaper, Assigned<R>),
    Record(DomainRecordCleanup<'l>, Assigned<R>),
    Policy(Assigned<R>),
    PolicyRetry(BoundCleanup, Retained<R>),
    InstallationRetry(Box<InstallationFailure>, Retained<R>),
    Pins(PinCleanup, Retained<R>),
    Unmount(Retained<R>),
    UnmountRetry(R::UnmountFailure),
    Complete(R::Unmounted),
}
/// Cleanup-only owner. No retry can restart launch or renew its fixed budget.
/// Dropping before quiescence retains policy and storage for VM disposal.
/// ```compile_fail
/// use conglobate::step::PreparationFailure;
/// fn cannot_extract_live_storage(failure: PreparationFailure<'_, '_, '_, '_>) {
///     failure.into_parts();
/// }
/// ```
/// ```compile_fail
/// use conglobate::step::PreparationFailure;
/// fn cannot_renew_cleanup_budget(failure: PreparationFailure<'_, '_, '_, '_>) {
///     failure.rollback(std::time::Instant::now());
/// }
/// ```
#[derive(Debug, thiserror::Error)]
#[error("command preparation: {diagnostics}")]
pub struct StoragePreparationFailure<'l, R: PreparationStorage> {
    diagnostics: FailureDiagnostics,
    phase: Phase<'l, R>,
    parent: PathBuf,
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
impl<'l, R: PreparationStorage> StoragePreparationFailure<'l, R> {
    fn early(
        source: impl Into<FailureDiagnostics>,
        phase: Phase<'l, R>,
        parent: &Path,
    ) -> Box<Self> {
        Box::new(Self {
            diagnostics: source.into(),
            phase,
            parent: parent.to_owned(),
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
            parent,
            deadline,
        } = self;
        let next: Result<Phase<'l, R>, (FailureDiagnostics, Phase<'l, R>)> = match phase {
            Phase::MountPins(pins, reaper, failure) => match pins.run(deadline) {
                Ok(()) => Ok(Phase::MountDrain(reaper, failure)),
                Err(error) => Err((
                    error.source.into(),
                    Phase::MountPins(error.cleanup, reaper, failure),
                )),
            },
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
            Phase::EarlyPins(pins, reaper, volume) => match pins.run(deadline) {
                Ok(()) => Ok(Phase::EarlyDrain(reaper, volume)),
                Err(failure) => Err((
                    failure.source.into(),
                    Phase::EarlyPins(failure.cleanup, reaper, volume),
                )),
            },
            Phase::MountedPins(pins, reaper, volume) => match pins.run(deadline) {
                Ok(()) => Ok(Phase::MountedDrain(reaper, volume)),
                Err(failure) => Err((
                    failure.source.into(),
                    Phase::MountedPins(failure.cleanup, reaper, volume),
                )),
            },
            Phase::MountedDrain(reaper, volume) => match reaper.drain_until(deadline) {
                Ok(_) => Ok(Phase::Unmount(volume)),
                Err(failure) => {
                    let (source, reaper) = failure.into_parts();
                    Err((source.into(), Phase::MountedDrain(reaper, volume)))
                }
            },
            Phase::DomainSetup(rollback, reaper, assigned) => {
                match rollback.retry_until(&parent, deadline) {
                    Ok(()) => Ok(Phase::EmptyDrain(reaper, assigned)),
                    Err(failure) => {
                        let (error, rollback) = failure.into_parts();
                        Err((error.into(), Phase::DomainSetup(rollback, reaper, assigned)))
                    }
                }
            }
            Phase::EmptyDrain(reaper, assigned) => match reaper.drain_until(deadline) {
                Ok(_) => Ok(Phase::Policy(assigned)),
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
                Ok(record) => Ok(Phase::Drain(record, reaper, assigned)),
                Err(failure) => {
                    let (error, domain) = failure.into_parts();
                    Err((error.into(), Phase::Domain(domain, reaper, assigned)))
                }
            },
            Phase::Drain(record, reaper, assigned) => match reaper.drain_until(deadline) {
                Ok(_) => Ok(Phase::Record(record, assigned)),
                Err(failure) => {
                    let (error, reaper) = failure.into_parts();
                    Err((error.into(), Phase::Drain(record, reaper, assigned)))
                }
            },
            Phase::Record(record, assigned) => match record.finish(&parent) {
                Ok(()) => Ok(Phase::Policy(assigned)),
                Err(failure) => {
                    let (error, record) = failure.into_parts();
                    Err((error.into(), Phase::Record(record, assigned)))
                }
            },
            Phase::Policy(assigned) => match assigned.policy {
                PolicyAssignment::Uninstalled(installation) => Ok(Phase::Pins(
                    ManuallyDrop::into_inner(installation)
                        .into_identities()
                        .into(),
                    assigned.volume,
                )),
                PolicyAssignment::Bound(policy) => {
                    match ManuallyDrop::into_inner(*policy).close() {
                        Ok(identities) => Ok(Phase::Pins(identities.into(), assigned.volume)),
                        Err(failure) => {
                            let (error, policy) = failure.into_parts();
                            Err((error.into(), Phase::PolicyRetry(policy, assigned.volume)))
                        }
                    }
                }
                PolicyAssignment::Failed(failure) => {
                    match ManuallyDrop::into_inner(failure).cleanup() {
                        Ok(identities) => Ok(Phase::Pins(identities.into(), assigned.volume)),
                        Err(failure) => Err((
                            failure.diagnostics().latest_only(),
                            Phase::InstallationRetry(failure, assigned.volume),
                        )),
                    }
                }
            },
            Phase::InstallationRetry(failure, volume) => match failure.cleanup() {
                Ok(identities) => Ok(Phase::Pins(identities.into(), volume)),
                Err(failure) => Err((
                    failure.diagnostics().latest_only(),
                    Phase::InstallationRetry(failure, volume),
                )),
            },
            Phase::PolicyRetry(policy, volume) => match policy.retry() {
                Ok(identities) => Ok(Phase::Pins(identities.into(), volume)),
                Err(failure) => {
                    let (error, policy) = failure.into_parts();
                    Err((error.into(), Phase::PolicyRetry(policy, volume)))
                }
            },
            Phase::Pins(pins, volume) => match pins.run(deadline) {
                Ok(()) => Ok(Phase::Unmount(volume)),
                Err(failure) => Err((failure.source.into(), Phase::Pins(failure.cleanup, volume))),
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
                parent,
                deadline,
            }
            .rollback(),
            Err((error, phase)) => Err(Box::new(Self {
                diagnostics: diagnostics.followed_by(error),
                phase,
                parent,
                deadline,
            })),
        }
    }
}

/// Single-volume preparation uses the shared storage lifecycle.
pub type CommandPreparation<'l, 'n, 'o, 'w> = StoragePreparation<'l, Volume<'n, 'o, 'w, Mounted>>;
pub type PreparationFailure<'l, 'n, 'o, 'w> =
    StoragePreparationFailure<'l, Volume<'n, 'o, 'w, Mounted>>;
pub type RolledBackPreparation<'n, 'o, 'w> = StorageRollback<Volume<'n, 'o, 'w, Mounted>>;
/// Bootstrap preparation retains both roots through the same policy/process rollback.
pub type BootstrapPreparation<'l, 'n, 'o, 'w> =
    StoragePreparation<'l, BootstrapVolumes<'n, 'o, 'w, Mounted>>;
pub type BootstrapPreparationFailure<'l, 'n, 'o, 'w> =
    StoragePreparationFailure<'l, BootstrapVolumes<'n, 'o, 'w, Mounted>>;
