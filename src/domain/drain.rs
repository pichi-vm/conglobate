// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::ReapedHelper;
use crate::process::{ChildReaper, ReapedChildren};
use std::{io, process::ExitStatus, time::Instant};

/// Owns the helper's exit status and the supervisor's process-wide drain.
/// Installation must precede helper creation. Other individually owned children
/// must already be reaped; this cannot enforce that process-wide prerequisite.
///
/// ```compile_fail
/// use conglobate::domain::DescendantDrain;
/// fn cannot_finish_before_draining(owner: DescendantDrain) {
///     owner.finish();
/// }
/// ```
#[derive(Debug)]
pub struct DescendantDrain {
    helper: ReapedHelper,
    reaper: ChildReaper,
}
#[derive(Debug, thiserror::Error)]
#[error("helper descendant drain: {source}")]
pub struct DescendantDrainFailure {
    source: io::Error,
    drain: DescendantDrain,
}
impl DescendantDrainFailure {
    pub fn into_parts(self) -> (io::Error, DescendantDrain) {
        (self.source, self.drain)
    }
}
/// Domain cancellation, helper reaping and process-wide drain completed. This
/// is not a filesystem finalization receipt.
#[derive(Debug)]
pub struct DrainedHelper {
    helper: ReapedHelper,
    children: ReapedChildren,
}
impl ReapedHelper {
    pub fn with_reaper(self, reaper: ChildReaper) -> DescendantDrain {
        DescendantDrain {
            helper: self,
            reaper,
        }
    }
}
impl DescendantDrain {
    pub fn drain_until(self, deadline: Instant) -> Result<DrainedHelper, DescendantDrainFailure> {
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
impl DrainedHelper {
    pub fn finish(self) -> (ExitStatus, ReapedChildren) {
        (self.helper.finish(), self.children)
    }
}
