// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    Assigned, BootstrapPreparation, BootstrapPreparationFailure, CommandPreparation, Phase,
    PreparationFailure, PreparationStorage, StoragePreparation, StoragePreparationFailure,
};
use crate::{
    anchor::RecoveryDirectory,
    execution::HelperProgram,
    isolation::{IdRange, Mapped, UserNamespace, UserNamespaceFailure},
    process::{ChildReaper, LaunchFailure},
    step::{Retained, SetupBudget},
    storage::{BootstrapVolumes, Mounted, Unmounted, Volume},
};
use std::{io, mem::ManuallyDrop, path::Path, process::Command};

/// Deployment-owned paths. Each acquisition validates its protected filesystem
/// and mount namespace; these paths are never taken from a command recipe.
#[derive(Debug)]
pub(crate) struct SandboxDirectories<'l> {
    recovery: &'l RecoveryDirectory,
    mounts: &'l Path,
    domains: &'l Path,
}
impl<'l> SandboxDirectories<'l> {
    pub(crate) fn new(recovery: &'l RecoveryDirectory, mounts: &'l Path, domains: &'l Path) -> Self {
        Self {
            recovery,
            mounts,
            domains,
        }
    }
}
/// Immutable per-step identity allocation and trusted setup program. The caller
/// supplies disjoint UID/GID ranges; all commands still run as mapped root.
#[derive(Debug)]
pub(crate) struct CommandSandbox<'l> {
    program: HelperProgram,
    directories: SandboxDirectories<'l>,
    uid: IdRange,
    gid: IdRange,
}
impl<'l> CommandSandbox<'l> {
    pub(crate) fn new(
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
    pub(crate) fn prepare_network(
        &self,
        network: &crate::network::CommandNetworking<'_>,
        budget: &SetupBudget,
    ) -> io::Result<crate::network::CommandNetwork> {
        network.prepare(budget)
    }
    pub(crate) fn mount_parent(&self) -> &Path {
        self.directories.mounts
    }
    /// Construct immutable launch input before acquiring the command resources.
    pub(crate) fn receiver(&self, script: String, budget: &SetupBudget) -> io::Result<Command> {
        self.program.receiver(
            script,
            budget.cutoff()?,
            self.uid.first(),
            self.gid.first(),
            self.directories.mounts,
        )
    }
    pub(crate) fn prepare<'n, 'o, 'w>(
        &self,
        volume: Volume<'n, 'o, 'w, Unmounted>,
        reaper: ChildReaper,
        budget: &SetupBudget,
    ) -> Result<CommandPreparation<'n, 'o, 'w>, Box<PreparationFailure<'n, 'o, 'w>>> {
        self.prepare_storage::<Volume<'n, 'o, 'w, Mounted>>(volume, reaper, budget)
    }
    pub(crate) fn prepare_bootstrap<'n, 'o, 'w>(
        &self,
        volumes: BootstrapVolumes<'n, 'o, 'w, Unmounted>,
        reaper: ChildReaper,
        budget: &SetupBudget,
    ) -> Result<BootstrapPreparation<'n, 'o, 'w>, Box<BootstrapPreparationFailure<'n, 'o, 'w>>>
    {
        self.prepare_storage::<BootstrapVolumes<'n, 'o, 'w, Mounted>>(volumes, reaper, budget)
    }
    pub(crate) fn prepare_storage<R: PreparationStorage>(
        &self,
        volume: R::Unmounted,
        reaper: ChildReaper,
        budget: &SetupBudget,
    ) -> Result<StoragePreparation<R>, Box<StoragePreparationFailure<R>>> {
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
                    return Err(StoragePreparationFailure::early(source, phase));
                }
            };
        if let Err(error) = budget.check() {
            drop(namespace);
            return Err(StoragePreparationFailure::early(
                error,
                Phase::EarlyDrain(reaper, volume),
            ));
        }
        let volume = match R::mount(volume, directories.mounts, budget.deadline()) {
            Ok(volume) => volume,
            Err((source, failure)) => {
                drop(namespace);
                return Err(StoragePreparationFailure::early(
                    source,
                    Phase::MountDrain(reaper, failure),
                ));
            }
        };
        if let Err(source) = budget
            .check()
            .and_then(|_| volume.validate_command_root())
            .and_then(|_| volume.validate_ownership(self.uid, self.gid, budget.deadline()))
        {
            let assigned = Assigned {
                namespace: ManuallyDrop::new(namespace),
                volume: Retained::new(volume),
            };
            return Err(StoragePreparationFailure::early(
                source,
                Phase::EmptyDrain(reaper, assigned),
            ));
        }
        StoragePreparation::create(
            namespace,
            volume,
            reaper,
            directories.recovery,
            directories.domains,
            budget,
        )
    }
}
