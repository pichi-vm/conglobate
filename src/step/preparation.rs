// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Ownership across helper launch, before the ready Step exists.
use super::{Ready, Retained, Step, pins::PinCleanup, teardown::Deadline};
use crate::{
    domain::{DomainCancellation, DomainRecordCleanup, HelperLaunchFailure, RecordedDomain},
    policy::{BoundCleanup, BoundDisabled, InstallationFailure, LabelPolicy, PolicyInstallation},
    process::{ChildReaper, Identified, LaunchFailure, Process, Running},
    storage::{FailureDiagnostics, Mounted, Unmounted, Volume, VolumeUnmountFailure},
};
use std::{
    io,
    mem::ManuallyDrop,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

#[derive(Debug)]
enum PolicyAssignment {
    Bound(Box<ManuallyDrop<LabelPolicy<BoundDisabled>>>),
    Failed(ManuallyDrop<Box<InstallationFailure>>),
}
#[derive(Debug)]
struct Assigned<'n, 'o, 'w> {
    policy: PolicyAssignment,
    volume: Retained<Volume<'n, 'o, 'w, Mounted>>,
}
/// Bound policy, pinned identities and mounted storage stay together even if
/// helper startup fails before registration. The reaper must already be installed.
#[derive(Debug)]
pub struct CommandPreparation<'l, 'n, 'o, 'w> {
    policy: ManuallyDrop<LabelPolicy<BoundDisabled>>,
    volume: Retained<Volume<'n, 'o, 'w, Mounted>>,
    domain: RecordedDomain<'l>,
    reaper: ChildReaper,
}
impl<'l, 'n, 'o, 'w> CommandPreparation<'l, 'n, 'o, 'w> {
    pub fn install(
        installation: PolicyInstallation,
        volume: Volume<'n, 'o, 'w, Mounted>,
        domain: RecordedDomain<'l>,
        reaper: ChildReaper,
        budget: &super::SetupBudget,
        domain_parent: &Path,
    ) -> Result<Self, Box<PreparationFailure<'l, 'n, 'o, 'w>>> {
        match installation.install(budget.deadline()) {
            Ok(policy) => Ok(Self::new(policy, volume, domain, reaper)),
            Err(failure) => {
                let diagnostics = failure.diagnostics().latest_only();
                Err(Box::new(PreparationFailure {
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
        volume: Volume<'n, 'o, 'w, Mounted>,
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
    ) -> Result<Step<'l, Ready, Volume<'n, 'o, 'w, Mounted>>, Box<PreparationFailure<'l, 'n, 'o, 'w>>>
    {
        let Self {
            policy,
            volume,
            domain,
            reaper,
        } = self;
        match domain.launch(command, setup_deadline) {
            Ok(helper) => Ok(Step::new(ManuallyDrop::into_inner(policy), helper, reaper)
                .with_volume(volume.release())),
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
                Err(Box::new(PreparationFailure {
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
enum Phase<'l, 'n, 'o, 'w> {
    Running(
        Process<Running>,
        DomainCancellation<'l>,
        ChildReaper,
        Assigned<'n, 'o, 'w>,
    ),
    Identified(
        Process<Identified>,
        DomainCancellation<'l>,
        ChildReaper,
        Assigned<'n, 'o, 'w>,
    ),
    Domain(DomainCancellation<'l>, ChildReaper, Assigned<'n, 'o, 'w>),
    Drain(DomainRecordCleanup<'l>, ChildReaper, Assigned<'n, 'o, 'w>),
    Record(DomainRecordCleanup<'l>, Assigned<'n, 'o, 'w>),
    Policy(Assigned<'n, 'o, 'w>),
    PolicyRetry(BoundCleanup, Retained<Volume<'n, 'o, 'w, Mounted>>),
    InstallationRetry(
        Box<InstallationFailure>,
        Retained<Volume<'n, 'o, 'w, Mounted>>,
    ),
    Pins(PinCleanup, Retained<Volume<'n, 'o, 'w, Mounted>>),
    Unmount(Retained<Volume<'n, 'o, 'w, Mounted>>),
    UnmountRetry(VolumeUnmountFailure<'n, 'o, 'w>),
    Complete(Volume<'n, 'o, 'w, Unmounted>),
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
pub struct PreparationFailure<'l, 'n, 'o, 'w> {
    diagnostics: FailureDiagnostics,
    phase: Phase<'l, 'n, 'o, 'w>,
    parent: PathBuf,
    deadline: Instant,
}
/// Failed setup was completely rolled back. This is never a successful step.
#[derive(Debug)]
pub struct RolledBackPreparation<'n, 'o, 'w> {
    diagnostics: FailureDiagnostics,
    volume: Volume<'n, 'o, 'w, Unmounted>,
}
impl<'n, 'o, 'w> RolledBackPreparation<'n, 'o, 'w> {
    pub fn into_parts(self) -> (FailureDiagnostics, Volume<'n, 'o, 'w, Unmounted>) {
        (self.diagnostics, self.volume)
    }
}
impl<'l, 'n, 'o, 'w> PreparationFailure<'l, 'n, 'o, 'w> {
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn rollback(self) -> Result<RolledBackPreparation<'n, 'o, 'w>, Box<Self>> {
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
        let next: Result<Phase<'l, 'n, 'o, 'w>, (FailureDiagnostics, Phase<'l, 'n, 'o, 'w>)> =
            match phase {
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
                    Err(failure) => {
                        Err((failure.source.into(), Phase::Pins(failure.cleanup, volume)))
                    }
                },
                Phase::Unmount(volume) => match volume.release().unmount() {
                    Ok(volume) => Ok(Phase::Complete(volume)),
                    Err(failure) => Err((
                        failure.diagnostics().latest_only(),
                        Phase::UnmountRetry(failure),
                    )),
                },
                Phase::UnmountRetry(failure) => match failure.retry() {
                    Ok(volume) => Ok(Phase::Complete(volume)),
                    Err(failure) => Err((
                        failure.diagnostics().latest_only(),
                        Phase::UnmountRetry(failure),
                    )),
                },
                Phase::Complete(volume) => {
                    return Ok(RolledBackPreparation {
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
