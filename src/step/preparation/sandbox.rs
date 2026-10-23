// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    Assigned, BootstrapPreparation, BootstrapPreparationFailure, CommandPreparation, Phase,
    PolicyAssignment, PreparationFailure, PreparationStorage, StoragePreparation,
    StoragePreparationFailure,
};
use crate::{
    anchor::{CreateFailure, RecoveryDirectory},
    execution::HelperProgram,
    isolation::{IdRange, Mapped, UserNamespace, UserNamespaceFailure},
    policy::PolicyInstallation,
    process::{ChildReaper, LaunchFailure},
    step::{Retained, SetupBudget, pins::PinCleanup},
    storage::{BootstrapVolumes, Mounted, Unmounted, Volume},
};
use std::{io, mem::ManuallyDrop, path::Path, process::Command};

/// Deployment-owned paths. Each acquisition validates its protected filesystem
/// and mount namespace; these paths are never taken from a command recipe.
#[derive(Debug)]
pub struct SandboxDirectories<'l> {
    recovery: &'l RecoveryDirectory,
    mounts: &'l Path,
    policies: &'l Path,
    domains: &'l Path,
}
impl<'l> SandboxDirectories<'l> {
    pub fn new(
        recovery: &'l RecoveryDirectory,
        mounts: &'l Path,
        policies: &'l Path,
        domains: &'l Path,
    ) -> Self {
        Self {
            recovery,
            mounts,
            policies,
            domains,
        }
    }
}
/// Immutable per-step identity allocation and trusted setup program. The caller
/// supplies disjoint UID/GID ranges; all commands still run as mapped root.
#[derive(Debug)]
pub struct CommandSandbox<'l> {
    program: HelperProgram,
    directories: SandboxDirectories<'l>,
    uid: IdRange,
    gid: IdRange,
}
impl<'l> CommandSandbox<'l> {
    pub fn new(
        program: HelperProgram,
        directories: SandboxDirectories<'l>,
        uid: IdRange,
        gid: IdRange,
    ) -> Self {
        Self {
            program,
            directories,
            uid,
            gid,
        }
    }
    pub(crate) fn mount_parent(&self) -> &Path {
        self.directories.mounts
    }
    pub(crate) fn domain_parent(&self) -> &Path {
        self.directories.domains
    }
    /// Construct immutable launch input before acquiring the command resources.
    pub fn receiver(&self, script: String, budget: &SetupBudget) -> io::Result<Command> {
        self.program.receiver(
            script,
            budget.cutoff()?,
            self.uid.first(),
            self.gid.first(),
            self.directories.mounts,
        )
    }
    pub fn prepare<'n, 'o, 'w>(
        &self,
        volume: Volume<'n, 'o, 'w, Unmounted>,
        reaper: ChildReaper,
        budget: &SetupBudget,
    ) -> Result<CommandPreparation<'l, 'n, 'o, 'w>, Box<PreparationFailure<'l, 'n, 'o, 'w>>> {
        self.prepare_storage::<Volume<'n, 'o, 'w, Mounted>>(volume, reaper, budget)
    }
    pub fn prepare_bootstrap<'n, 'o, 'w>(
        &self,
        volumes: BootstrapVolumes<'n, 'o, 'w, Unmounted>,
        reaper: ChildReaper,
        budget: &SetupBudget,
    ) -> Result<
        BootstrapPreparation<'l, 'n, 'o, 'w>,
        Box<BootstrapPreparationFailure<'l, 'n, 'o, 'w>>,
    > {
        self.prepare_storage::<BootstrapVolumes<'n, 'o, 'w, Mounted>>(volumes, reaper, budget)
    }
    pub(crate) fn prepare_storage<R: PreparationStorage>(
        &self,
        volume: R::Unmounted,
        reaper: ChildReaper,
        budget: &SetupBudget,
    ) -> Result<StoragePreparation<'l, R>, Box<StoragePreparationFailure<'l, R>>> {
        let directories = &self.directories;
        let creator = match budget
            .cutoff()
            .and_then(|cutoff| self.program.namespace_creator(cutoff))
        {
            Ok(creator) => creator,
            Err(error) => {
                return Err(StoragePreparationFailure::early(
                    error,
                    Phase::EarlyDrain(reaper, volume),
                    directories.domains,
                ));
            }
        };
        let namespace =
            match UserNamespace::<Mapped>::create(creator, self.uid, self.gid, budget.deadline()) {
                Ok(namespace) => namespace,
                Err(failure) => {
                    let (source, phase) = match failure {
                        UserNamespaceFailure::Launch(LaunchFailure::Before(source)) => {
                            (source, Phase::EarlyDrain(reaper, volume))
                        }
                        UserNamespaceFailure::Launch(LaunchFailure::Started {
                            source,
                            process,
                        }) => (source, Phase::EarlyRunning(process, reaper, volume)),
                        UserNamespaceFailure::Mapping { source, process } => {
                            (source, Phase::EarlyIdentified(process, reaper, volume))
                        }
                        UserNamespaceFailure::Reap(failure) => {
                            let (source, process) = failure.into_parts();
                            (
                                io::Error::other(source),
                                Phase::EarlyIdentified(process, reaper, volume),
                            )
                        }
                        failure @ UserNamespaceFailure::Exited(_) => {
                            (io::Error::other(failure), Phase::EarlyDrain(reaper, volume))
                        }
                    };
                    return Err(StoragePreparationFailure::early(
                        source,
                        phase,
                        directories.domains,
                    ));
                }
            };
        if let Err(error) = budget.check() {
            drop(namespace);
            return Err(StoragePreparationFailure::early(
                error,
                Phase::EarlyDrain(reaper, volume),
                directories.domains,
            ));
        }
        let namespace = match namespace.pin(directories.recovery) {
            Ok(namespace) => namespace,
            Err(CreateFailure::Before(source)) => {
                return Err(StoragePreparationFailure::early(
                    source,
                    Phase::EarlyDrain(reaper, volume),
                    directories.domains,
                ));
            }
            Err(CreateFailure::Mount { source, target }) => {
                return Err(StoragePreparationFailure::early(
                    source,
                    Phase::EarlyPins(PinCleanup::target(target), reaper, volume),
                    directories.domains,
                ));
            }
        };
        if let Err(error) = budget.check() {
            return Err(StoragePreparationFailure::early(
                error,
                Phase::EarlyPins(PinCleanup::namespace(namespace), reaper, volume),
                directories.domains,
            ));
        }
        let volume = match R::mount(volume, directories.mounts, budget.deadline()) {
            Ok(volume) => volume,
            Err((source, failure)) => {
                return Err(StoragePreparationFailure::early(
                    source,
                    Phase::MountPins(PinCleanup::namespace(namespace), reaper, failure),
                    directories.domains,
                ));
            }
        };
        if let Err(source) = budget.check().and_then(|_| volume.validate_command_root()) {
            let assigned = Assigned {
                policy: PolicyAssignment::Uninstalled(ManuallyDrop::new(Box::new(
                    PolicyInstallation::new(namespace, vec![], directories.policies),
                ))),
                volume: Retained::new(volume),
            };
            return Err(StoragePreparationFailure::early(
                source,
                Phase::EmptyDrain(reaper, assigned),
                directories.domains,
            ));
        }
        let pins = match volume.pin(directories.recovery) {
            Ok(pins) => pins,
            Err(failure) => {
                let (source, pins) = failure.into_cleanup(namespace);
                return Err(StoragePreparationFailure::early(
                    source,
                    Phase::MountedPins(pins, reaper, Retained::new(volume)),
                    directories.domains,
                ));
            }
        };
        StoragePreparation::create(
            PolicyInstallation::new(namespace, pins, directories.policies),
            volume,
            reaper,
            directories.recovery,
            directories.domains,
            budget,
        )
    }
}
