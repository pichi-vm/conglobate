// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Assigned, CommandStorage, StoragePreparation};
#[cfg(feature = "test-fixtures")]
use super::{BootstrapPreparation, CommandPreparation};
#[cfg(feature = "test-fixtures")]
use crate::storage::{BootstrapVolumes, Mounted, Unmounted, Volume};
use crate::{
    anchor::RecoveryDirectory,
    execution::HelperProgram,
    isolation::{IdRange, Mapped, UserNamespace},
    process::ChildReaper,
    step::{Retained, SetupBudget},
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
    pub(crate) fn new(
        recovery: &'l RecoveryDirectory,
        mounts: &'l Path,
        domains: &'l Path,
    ) -> Self {
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
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn prepare<'n, 'o, 'w>(
        &self,
        volume: Volume<'n, 'o, 'w, Unmounted>,
        reaper: ChildReaper,
        budget: &SetupBudget,
    ) -> anyhow::Result<CommandPreparation<'n, 'o, 'w>> {
        self.prepare_storage::<Volume<'n, 'o, 'w, Mounted>>(volume, reaper, budget)
    }
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn prepare_bootstrap<'n, 'o, 'w>(
        &self,
        volumes: BootstrapVolumes<'n, 'o, 'w, Unmounted>,
        reaper: ChildReaper,
        budget: &SetupBudget,
    ) -> anyhow::Result<BootstrapPreparation<'n, 'o, 'w>> {
        self.prepare_storage::<BootstrapVolumes<'n, 'o, 'w, Mounted>>(volumes, reaper, budget)
    }
    pub(crate) fn prepare_storage<R: CommandStorage>(
        &self,
        volume: R::Unmounted,
        reaper: ChildReaper,
        budget: &SetupBudget,
    ) -> anyhow::Result<StoragePreparation<R>> {
        let creator = self.program.namespace_creator(budget.cutoff()?)?;
        let namespace =
            match UserNamespace::<Mapped>::create(creator, self.uid, self.gid, budget.deadline()) {
                Ok(namespace) => namespace,
                Err(failure) => return Err(failure.reject(reaper)),
            };
        budget.check()?;
        let volume = R::mount(volume, self.directories.mounts, budget.deadline())?;
        if let Err(source) = budget
            .check()
            .and_then(|_| volume.validate_command_root())
            .and_then(|_| volume.validate_ownership(self.uid, self.gid, budget.deadline()))
        {
            let assigned = Assigned {
                namespace: ManuallyDrop::new(namespace),
                volume: Retained::new(volume),
            };
            return Err(assigned.reject(
                reaper,
                source.into(),
                std::time::Instant::now() + std::time::Duration::from_secs(30),
            ));
        }
        StoragePreparation::create(
            namespace,
            volume,
            reaper,
            self.directories.recovery,
            self.directories.domains,
            budget,
        )
    }
}
