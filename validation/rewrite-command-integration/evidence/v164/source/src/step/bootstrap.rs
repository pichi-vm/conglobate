// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    Ready, Retained, Step, StepStorage, private,
    teardown::{Deadline, TeardownCompletion},
};
use crate::{
    process::ReapedChildren,
    storage::{BootstrapUnmountFailure, BootstrapVolumes, FailureDiagnostics, Mounted, Unmounted},
};
use std::{marker::PhantomData, process::ExitStatus, time::Instant};
impl private::Storage for BootstrapVolumes<'_, '_, '_, Mounted> {}
impl StepStorage for BootstrapVolumes<'_, '_, '_, Mounted> {}
impl<'l> Step<'l, Ready> {
    /// Consume both mounted owners before commands are admitted. Neither can
    /// be extracted until descendant drain, policy removal and finalization.
    pub fn with_bootstrap<'n, 'o, 'w>(
        self,
        volumes: BootstrapVolumes<'n, 'o, 'w, Mounted>,
    ) -> Step<'l, Ready, BootstrapVolumes<'n, 'o, 'w, Mounted>> {
        Step {
            storage: Retained::new(volumes),
            policy: self.policy,
            resources: self.resources,
            _lease: PhantomData,
        }
    }
}
#[derive(Debug)]
enum Pending<'n, 'o, 'w> {
    Mounted(BootstrapVolumes<'n, 'o, 'w, Mounted>),
    Retry(Box<BootstrapUnmountFailure<'n, 'o, 'w>>),
    Complete(BootstrapVolumes<'n, 'o, 'w, Unmounted>),
}
#[derive(Debug)]
struct Finalization<'n, 'o, 'w> {
    pending: Pending<'n, 'o, 'w>,
    deadline: Instant,
    status: ExitStatus,
    reaped: ReapedChildren,
}
/// Both filesystems were flushed/unmounted and both snapshot statuses checked
/// under the original teardown cutoff. This does not assert command success.
#[derive(Debug)]
pub struct FinalizedBootstrap<'n, 'o, 'w> {
    volumes: BootstrapVolumes<'n, 'o, 'w, Unmounted>,
    status: ExitStatus,
    reaped: ReapedChildren,
}
impl<'n, 'o, 'w> FinalizedBootstrap<'n, 'o, 'w> {
    pub fn into_parts(
        self,
    ) -> (
        ExitStatus,
        ReapedChildren,
        BootstrapVolumes<'n, 'o, 'w, Unmounted>,
    ) {
        (self.status, self.reaped, self.volumes)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("bootstrap storage finalization: {diagnostics}")]
pub struct BootstrapFinalizationFailure<'n, 'o, 'w> {
    diagnostics: FailureDiagnostics,
    finalization: Finalization<'n, 'o, 'w>,
}
impl<'n, 'o, 'w> BootstrapFinalizationFailure<'n, 'o, 'w> {
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn retry(self) -> Result<FinalizedBootstrap<'n, 'o, 'w>, Box<Self>> {
        self.finalization.run().map_err(|failure| {
            Box::new(Self {
                diagnostics: self.diagnostics.followed_by(failure.diagnostics),
                finalization: failure.finalization,
            })
        })
    }
}
impl<'n, 'o, 'w> Finalization<'n, 'o, 'w> {
    fn run(
        self,
    ) -> Result<FinalizedBootstrap<'n, 'o, 'w>, Box<BootstrapFinalizationFailure<'n, 'o, 'w>>> {
        if let Err(error) = Deadline::check(self.deadline) {
            return Err(Box::new(BootstrapFinalizationFailure {
                diagnostics: error.into(),
                finalization: self,
            }));
        }
        let Self {
            pending,
            deadline,
            status,
            reaped,
        } = self;
        let result = match pending {
            Pending::Mounted(volumes) => volumes.unmount(),
            Pending::Retry(failure) => failure.retry(),
            Pending::Complete(volumes) => {
                if let Err(error) = volumes
                    .validate_snapshots()
                    .and_then(|()| Deadline::check(deadline))
                {
                    return Err(Box::new(BootstrapFinalizationFailure {
                        diagnostics: error.into(),
                        finalization: Self {
                            pending: Pending::Complete(volumes),
                            deadline,
                            status,
                            reaped,
                        },
                    }));
                }
                return Ok(FinalizedBootstrap {
                    volumes,
                    status,
                    reaped,
                });
            }
        };
        match result {
            Ok(volumes) => Self {
                pending: Pending::Complete(volumes),
                deadline,
                status,
                reaped,
            }
            .run(),
            Err(failure) => Err(Box::new(BootstrapFinalizationFailure {
                diagnostics: failure.diagnostics().latest_only(),
                finalization: Self {
                    pending: Pending::Retry(failure),
                    deadline,
                    status,
                    reaped,
                },
            })),
        }
    }
}
impl<'n, 'o, 'w> TeardownCompletion<BootstrapVolumes<'n, 'o, 'w, Mounted>> {
    pub fn unmount(
        self,
    ) -> Result<FinalizedBootstrap<'n, 'o, 'w>, Box<BootstrapFinalizationFailure<'n, 'o, 'w>>> {
        Finalization {
            pending: Pending::Mounted(self.released.storage),
            deadline: self.deadline,
            status: self.released.status,
            reaped: self.released.reaped,
        }
        .run()
    }
}
