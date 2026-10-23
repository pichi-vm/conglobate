// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    PreparationStorage,
    teardown::{Deadline, TeardownCompletion},
};
use crate::{
    process::ReapedChildren,
    storage::{BootstrapVolumes, FailureDiagnostics, Mounted, Volume},
};
use std::{process::ExitStatus, time::Instant};

#[derive(Debug)]
enum Pending<R: PreparationStorage> {
    Mounted(R),
    Retry(R::UnmountFailure),
    Complete(R::Unmounted),
}
#[derive(Debug)]
struct Finalization<R: PreparationStorage> {
    pending: Pending<R>,
    deadline: Instant,
    status: ExitStatus,
    reaped: ReapedChildren,
}
/// Storage was flushed, cleanly unmounted, and its snapshot status validated
/// within the original teardown budget. The status is the outer trusted helper's exit status, not the
/// command outcome. This is not a sealed-image receipt.
#[derive(Debug)]
pub struct FinalizedStorage<R: PreparationStorage> {
    volume: R::Unmounted,
    status: ExitStatus,
    reaped: ReapedChildren,
}
impl<R: PreparationStorage> FinalizedStorage<R> {
    pub fn into_parts(self) -> (ExitStatus, ReapedChildren, R::Unmounted) {
        (self.status, self.reaped, self.volume)
    }
}
/// Failed finalization retains storage and the original teardown cutoff.
/// ```compile_fail
/// use conglobate::step::FinalizationFailure;
/// fn cannot_extend_finalization(failure: FinalizationFailure<'_, '_, '_>) {
///     failure.retry(std::time::Instant::now());
/// }
/// ```
#[derive(Debug, thiserror::Error)]
#[error("step storage finalization: {diagnostics}")]
pub struct StorageFinalizationFailure<R: PreparationStorage> {
    diagnostics: FailureDiagnostics,
    finalization: Finalization<R>,
}
impl<R: PreparationStorage> StorageFinalizationFailure<R> {
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn retry(self) -> Result<FinalizedStorage<R>, Box<Self>> {
        self.finalization.run().map_err(|failure| {
            let Self {
                diagnostics,
                finalization,
            } = *failure;
            Box::new(Self {
                diagnostics: self.diagnostics.followed_by(diagnostics),
                finalization,
            })
        })
    }
}
impl<R: PreparationStorage> Finalization<R> {
    fn run(self) -> Result<FinalizedStorage<R>, Box<StorageFinalizationFailure<R>>> {
        if let Err(error) = Deadline::check(self.deadline) {
            return Err(Box::new(StorageFinalizationFailure {
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
            Pending::Mounted(volume) => volume.unmount_until(deadline),
            Pending::Retry(failure) => R::retry_unmount(failure),
            Pending::Complete(volume) => {
                if let Err(error) = R::validate(&volume).and_then(|()| Deadline::check(deadline)) {
                    return Err(Box::new(StorageFinalizationFailure {
                        diagnostics: error.into(),
                        finalization: Self {
                            pending: Pending::Complete(volume),
                            deadline,
                            status,
                            reaped,
                        },
                    }));
                }
                return Ok(FinalizedStorage {
                    volume,
                    status,
                    reaped,
                });
            }
        };
        match result {
            Ok(volume) => Self {
                pending: Pending::Complete(volume),
                deadline,
                status,
                reaped,
            }
            .run(),
            Err(failure) => {
                let diagnostics = R::unmount_diagnostics(&failure).latest_only();
                Err(Box::new(StorageFinalizationFailure {
                    diagnostics,
                    finalization: Self {
                        pending: Pending::Retry(failure),
                        deadline,
                        status,
                        reaped,
                    },
                }))
            }
        }
    }
}
impl<R: PreparationStorage> TeardownCompletion<R> {
    /// Continue with the existing deadline; no new finalization budget is minted.
    pub fn unmount(self) -> Result<FinalizedStorage<R>, Box<StorageFinalizationFailure<R>>> {
        Finalization {
            pending: Pending::Mounted(self.released.storage),
            deadline: self.deadline,
            status: self.released.status,
            reaped: self.released.reaped,
        }
        .run()
    }
}
impl TeardownCompletion<()> {
    pub fn into_parts(self) -> (ExitStatus, ReapedChildren) {
        (self.released.status, self.released.reaped)
    }
}

impl<R: PreparationStorage> super::CommandResult<TeardownCompletion<R>> {
    pub fn unmount(
        self,
    ) -> Result<
        super::CommandResult<FinalizedStorage<R>>,
        super::CommandResult<Box<StorageFinalizationFailure<R>>>,
    > {
        self.map_cleanup(|completion| completion.unmount())
            .transpose()
    }
}
impl<R: PreparationStorage> super::CommandResult<Box<StorageFinalizationFailure<R>>> {
    /// Preserve both primary outcome and the existing finalization deadline.
    pub fn retry(self) -> Result<super::CommandResult<FinalizedStorage<R>>, Self> {
        self.map_cleanup(|failure| failure.retry()).transpose()
    }
}

/// Finalized storage for a single command root.
pub type FinalizedVolume<'n, 'o, 'w> = FinalizedStorage<Volume<'n, 'o, 'w, Mounted>>;
/// Failed finalization retaining a single command root.
pub type FinalizationFailure<'n, 'o, 'w> = StorageFinalizationFailure<Volume<'n, 'o, 'w, Mounted>>;
/// Both bootstrap filesystems were flushed, unmounted and snapshot-validated.
pub type FinalizedBootstrap<'n, 'o, 'w> = FinalizedStorage<BootstrapVolumes<'n, 'o, 'w, Mounted>>;
/// Failed finalization retaining both bootstrap filesystem owners.
pub type BootstrapFinalizationFailure<'n, 'o, 'w> =
    StorageFinalizationFailure<BootstrapVolumes<'n, 'o, 'w, Mounted>>;
