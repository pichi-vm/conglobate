// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::storage::{BootstrapVolumes, Unmounted, Volume};

pub(super) trait DiscardStorage {
    fn discard(self) -> anyhow::Result<()>;
}
impl DiscardStorage for Volume<'_, '_, '_, Unmounted> {
    fn discard(self) -> anyhow::Result<()> {
        self.into_snapshot()?.close()?.detach()?;
        Ok(())
    }
}
impl DiscardStorage for BootstrapVolumes<'_, '_, '_, Unmounted> {
    fn discard(self) -> anyhow::Result<()> {
        let (tool, output) = self.into_parts();
        // Attempt both closes; one failure must not suppress the other cleanup.
        let tool = tool.discard();
        let output = output.discard();
        match (tool, output) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(tool), Err(output)) => {
                Err(tool.context(format!("output cleanup also failed: {output:#}")))
            }
        }
    }
}
