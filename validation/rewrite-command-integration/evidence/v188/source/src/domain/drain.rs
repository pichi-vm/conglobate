// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::ReapedHelper;
use crate::process::{ChildReaper, ReapedChildren};
use std::{io, path::Path, process::ExitStatus, time::Instant};

/// Owns the helper's remaining record and the supervisor's process-wide drain.
/// Installation must precede helper creation. Other individually owned children
/// must already be reaped; this cannot enforce that process-wide prerequisite.
///
/// ```compile_fail
/// use conglobate::domain::DescendantDrain;
/// fn cannot_finish_before_draining(owner: DescendantDrain<'_>) {
///     owner.finish(std::path::Path::new("/unused"));
/// }
/// ```
#[derive(Debug)]
pub struct DescendantDrain<'lease> {
    helper: ReapedHelper<'lease>,
    reaper: ChildReaper,
}
#[derive(Debug, thiserror::Error)]
#[error("helper descendant drain: {source}")]
pub struct DescendantDrainFailure<'lease> {
    source: io::Error,
    drain: DescendantDrain<'lease>,
}
impl<'lease> DescendantDrainFailure<'lease> {
    pub fn into_parts(self) -> (io::Error, DescendantDrain<'lease>) {
        (self.source, self.drain)
    }
}
/// Domain cancellation, helper reaping and process-wide drain completed. This
/// still owns the protected record and is not a policy/mount completion receipt.
#[derive(Debug)]
pub struct DrainedHelper<'lease> {
    helper: ReapedHelper<'lease>,
    children: ReapedChildren,
}
#[derive(Debug, thiserror::Error)]
#[error("drained helper record cleanup: {source}")]
pub struct DrainedFinishFailure<'lease> {
    source: io::Error,
    helper: DrainedHelper<'lease>,
}
impl<'lease> DrainedFinishFailure<'lease> {
    pub fn into_parts(self) -> (io::Error, DrainedHelper<'lease>) {
        (self.source, self.helper)
    }
}
impl<'lease> ReapedHelper<'lease> {
    pub fn with_reaper(self, reaper: ChildReaper) -> DescendantDrain<'lease> {
        DescendantDrain {
            helper: self,
            reaper,
        }
    }
}
impl<'lease> DescendantDrain<'lease> {
    pub fn drain_until(
        self,
        deadline: Instant,
    ) -> Result<DrainedHelper<'lease>, DescendantDrainFailure<'lease>> {
        let Self { helper, reaper } = self;
        match reaper.drain_until(deadline) {
            Ok(children) => Ok(DrainedHelper { helper, children }),
            Err(failure) => {
                let (source, reaper) = failure.into_parts();
                Err(DescendantDrainFailure {
                    source,
                    drain: Self { helper, reaper },
                })
            }
        }
    }
}
impl<'lease> DrainedHelper<'lease> {
    pub fn finish(
        self,
        parent: &Path,
    ) -> Result<(ExitStatus, ReapedChildren), DrainedFinishFailure<'lease>> {
        let Self { helper, children } = self;
        match helper.finish(parent) {
            Ok(status) => Ok((status, children)),
            Err(failure) => {
                let (source, helper) = failure.into_parts();
                Err(DrainedFinishFailure {
                    source,
                    helper: Self { helper, children },
                })
            }
        }
    }
}
