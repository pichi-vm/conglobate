// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    cache::{Cache, FromIndex as _},
    input::CheckedImage,
    step::SetupBudget,
    storage::{Created, Mapping, Mounted, ReadOnlyChain, Runtime, SealedScute, Volume, Workspace},
};
use std::{num::NonZeroU64, sync::Arc};

#[derive(Debug, Clone)]
pub(crate) struct Root<'b, 'w> {
    pub(crate) base: Option<&'b CheckedImage>,
    pub(crate) produced: Vec<Arc<SealedScute<'w>>>,
}
impl<'w> Root<'_, 'w> {
    pub(crate) fn hash(&self) -> [u8; 32] {
        self.produced
            .last()
            .map(|layer| *layer.root())
            .or_else(|| self.base.and_then(CheckedImage::root))
            .expect("selected root")
    }
    pub(super) fn activate(&self, cache: &Cache) -> anyhow::Result<ReadOnlyChain<'w>> {
        let base = self
            .base
            .map(|image| carapace::Chain::from_index(cache, image.image()))
            .transpose()?;
        Ok(ReadOnlyChain::assemble(base, &self.produced)?)
    }
    /// Readers mount only the persisted root, with disposable journal writes.
    /// No stage mounts are inherited or recreated in this namespace.
    pub(super) fn read<T>(
        &self,
        cache: &Cache,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        operation: impl FnOnce(&mut Volume<'_, '_, 'w, Mounted>, &SetupBudget) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let budget = SetupBudget::begin();
        let active = self.activate(cache)?;
        let snapshot = Mapping::<Created>::create()?.snapshot(
            &active,
            workspace.snapshot_store()?.attach()?,
            NonZeroU64::new(carapace::ZERO_SIZE_SECTORS).unwrap(),
        )?;
        let paths = runtime.paths()?;
        let mut volume =
            Volume::in_init_namespace(snapshot, runtime.recovery())?.mount_ext4(paths.volumes())?;
        let result = budget
            .check()
            .map_err(Into::into)
            .and_then(|()| operation(&mut volume, &budget));
        let cleanup = (|| -> anyhow::Result<()> {
            volume.unmount()?.into_snapshot()?.close()?.detach()?;
            Ok(())
        })()
        .and_then(|()| active.close().map_err(anyhow::Error::from));
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => {
                Err(error.context(format!("source reader cleanup: {cleanup}")))
            }
        }
    }
}
