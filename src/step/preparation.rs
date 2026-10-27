// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

mod sandbox;
use super::CommandStorage;
use super::{Ready, Retained, Step};
#[cfg(feature = "test-fixtures")]
use crate::storage::{BootstrapVolumes, Mounted, Volume};
use crate::{
    domain::{CommandDomain, DomainCreateFailure},
    isolation::{Mapped, UserNamespace},
    process::ChildReaper,
};
pub(crate) use sandbox::{CommandSandbox, SandboxDirectories};
use std::{
    mem::ManuallyDrop,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

/// Assigned resources remain retained if any cleanup cannot prove quiescence.
#[derive(Debug)]
struct Assigned<R: CommandStorage> {
    namespace: ManuallyDrop<UserNamespace<Mapped>>,
    volume: Retained<R>,
}
impl<R: CommandStorage> Assigned<R> {
    fn reject(self, reaper: ChildReaper, error: anyhow::Error, deadline: Instant) -> anyhow::Error {
        if let Err(failure) = reaper.drain_until(deadline) {
            let (cleanup, _) = failure.into_parts();
            return error.context(format!("child drain: {cleanup}"));
        }
        drop(ManuallyDrop::into_inner(self.namespace));
        match self.volume.release().unmount_until(deadline) {
            Ok(storage) => {
                drop(storage);
                error
            }
            Err(failure) => error.context(format!("storage cleanup: {failure}")),
        }
    }
}
#[derive(Debug)]
pub(crate) struct StoragePreparation<R: CommandStorage> {
    assigned: Assigned<R>,
    domain: CommandDomain,
    reaper: ChildReaper,
}
impl<R: CommandStorage> StoragePreparation<R> {
    pub(crate) fn create(
        namespace: UserNamespace<Mapped>,
        volume: R,
        reaper: ChildReaper,
        recovery: &crate::anchor::RecoveryDirectory,
        domain_parent: &Path,
        budget: &super::SetupBudget,
    ) -> anyhow::Result<Self> {
        let assigned = Assigned {
            namespace: ManuallyDrop::new(namespace),
            volume: Retained::new(volume),
        };
        let domain = budget
            .check()
            .map_err(DomainCreateFailure::Before)
            .and_then(|()| CommandDomain::create(recovery, domain_parent));
        match domain {
            Ok(domain) => Ok(Self {
                assigned,
                domain,
                reaper,
            }),
            Err(failure) => {
                let deadline = Instant::now() + Duration::from_secs(30);
                let (source, rollback) = failure.into_rollback();
                let error = anyhow::Error::new(source);
                if let Err(failure) = rollback.retry_until(deadline) {
                    let error = error.context(format!("domain setup cleanup: {failure}"));
                    return Err(error);
                }
                Err(assigned.reject(reaper, error, deadline))
            }
        }
    }
    pub(crate) fn launch<'l>(
        self,
        command: Command,
        setup_deadline: Instant,
    ) -> anyhow::Result<Step<'l, Ready, R>> {
        let Self {
            assigned,
            domain,
            reaper,
        } = self;
        match domain.launch(command, setup_deadline) {
            Ok(helper) => {
                Ok(
                    Step::new(ManuallyDrop::into_inner(assigned.namespace), helper, reaper)
                        .with_storage(assigned.volume.release()),
                )
            }
            Err(failure) => {
                let deadline = Instant::now() + Duration::from_secs(30);
                let (source, cleanup) = failure.cancel(deadline);
                let error = anyhow::Error::new(source);
                match cleanup {
                    Ok(()) => Err(assigned.reject(reaper, error, deadline)),
                    // The assigned guard retains roots for disposal.
                    Err(cleanup) => Err(error.context(format!("launch cleanup: {cleanup:#}"))),
                }
            }
        }
    }
}
#[cfg(feature = "test-fixtures")]
pub(crate) type CommandPreparation<'n, 'o, 'w> = StoragePreparation<Volume<'n, 'o, 'w, Mounted>>;
#[cfg(feature = "test-fixtures")]
pub(crate) type BootstrapPreparation<'n, 'o, 'w> =
    StoragePreparation<BootstrapVolumes<'n, 'o, 'w, Mounted>>;
