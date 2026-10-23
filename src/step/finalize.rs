// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::teardown::{Deadline, TeardownCompletion};
use crate::{
    process::ReapedChildren,
    storage::{FailureDiagnostics, Mounted, Unmounted, Volume, VolumeUnmountFailure},
};
use std::{process::ExitStatus, time::Instant};

#[derive(Debug)]
enum Pending<'ns, 'origin, 'workspace> {
    Mounted(Volume<'ns, 'origin, 'workspace, Mounted>),
    Retry(VolumeUnmountFailure<'ns, 'origin, 'workspace>),
    Complete(Volume<'ns, 'origin, 'workspace, Unmounted>),
}
#[derive(Debug)]
struct Finalization<'ns, 'origin, 'workspace> {
    pending: Pending<'ns, 'origin, 'workspace>,
    deadline: Instant,
    status: ExitStatus,
    reaped: ReapedChildren,
}
/// Storage was flushed and cleanly unmounted within the original teardown
/// budget. Command status is diagnostic; this is not a sealed-image receipt.
#[derive(Debug)]
pub struct FinalizedVolume<'ns, 'origin, 'workspace> {
    volume: Volume<'ns, 'origin, 'workspace, Unmounted>,
    status: ExitStatus,
    reaped: ReapedChildren,
}
impl<'ns, 'origin, 'workspace> FinalizedVolume<'ns, 'origin, 'workspace> {
    pub fn into_parts(
        self,
    ) -> (
        ExitStatus,
        ReapedChildren,
        Volume<'ns, 'origin, 'workspace, Unmounted>,
    ) {
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
pub struct FinalizationFailure<'ns, 'origin, 'workspace> {
    diagnostics: FailureDiagnostics,
    finalization: Finalization<'ns, 'origin, 'workspace>,
}
impl<'ns, 'origin, 'workspace> FinalizationFailure<'ns, 'origin, 'workspace> {
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn retry(self) -> Result<FinalizedVolume<'ns, 'origin, 'workspace>, Box<Self>> {
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
impl<'ns, 'origin, 'workspace> Finalization<'ns, 'origin, 'workspace> {
    fn run(
        self,
    ) -> Result<
        FinalizedVolume<'ns, 'origin, 'workspace>,
        Box<FinalizationFailure<'ns, 'origin, 'workspace>>,
    > {
        if let Err(error) = Deadline::check(self.deadline) {
            return Err(Box::new(FinalizationFailure {
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
            Pending::Mounted(volume) => volume.unmount(),
            Pending::Retry(failure) => failure.retry(),
            Pending::Complete(volume) => {
                return Ok(FinalizedVolume {
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
                let diagnostics = failure.diagnostics().latest_only();
                Err(Box::new(FinalizationFailure {
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
impl<'ns, 'origin, 'workspace> TeardownCompletion<Volume<'ns, 'origin, 'workspace, Mounted>> {
    /// Continue with the existing deadline; no new finalization budget is minted.
    pub fn unmount(
        self,
    ) -> Result<
        FinalizedVolume<'ns, 'origin, 'workspace>,
        Box<FinalizationFailure<'ns, 'origin, 'workspace>>,
    > {
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
