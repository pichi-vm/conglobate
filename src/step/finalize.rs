// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{
    CommandStorage,
    teardown::{Deadline, TeardownCompletion},
};
use crate::process::ReapedChildren;
use std::process::ExitStatus;
#[derive(Debug)]
pub(crate) struct FinalizedStorage<R: CommandStorage> {
    volume: R::Unmounted,
    status: ExitStatus,
    reaped: ReapedChildren,
}
impl<R: CommandStorage> FinalizedStorage<R> {
    pub(crate) fn into_parts(self) -> (ExitStatus, ReapedChildren, R::Unmounted) {
        (self.status, self.reaped, self.volume)
    }
}
impl<R: CommandStorage> TeardownCompletion<R> {
    pub(crate) fn unmount(self) -> anyhow::Result<FinalizedStorage<R>> {
        if let Err(error) = Deadline::check(self.deadline) {
            std::mem::forget(self);
            return Err(error.into());
        }
        let volume = self.released.storage.unmount_until(self.deadline)?;
        R::validate(&volume)?;
        Deadline::check(self.deadline)?;
        Ok(FinalizedStorage {
            volume,
            status: self.released.status,
            reaped: self.released.reaped,
        })
    }
}
#[cfg(feature = "test-fixtures")]
impl<R: CommandStorage> super::outcome::CommandResult<TeardownCompletion<R>> {
    pub(crate) fn unmount(
        self,
    ) -> Result<
        super::outcome::CommandResult<FinalizedStorage<R>>,
        super::outcome::CommandResult<anyhow::Error>,
    > {
        self.map_cleanup(|completion| completion.unmount())
            .transpose()
    }
}
