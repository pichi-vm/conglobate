// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    artifact::PmiDescriptor, cache::Cache, firmware::FirmwareFile, input::CheckedImage,
    output::StagedDirectory,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) enum Pmi<'b, 'w> {
    Inherited(&'b CheckedImage),
    File(Arc<FirmwareFile<'w>>),
}
impl Pmi<'_, '_> {
    pub(crate) fn descriptor(&self) -> PmiDescriptor {
        match self {
            Self::Inherited(image) => image
                .image()
                .manifest()
                .layers
                .iter()
                .find_map(|layer| match layer {
                    crate::artifact::Layer::Pmi(descriptor) => Some(descriptor.clone()),
                    _ => None,
                })
                .expect("selected PMI"),
            Self::File(file) => file.descriptor(),
        }
    }
    pub(super) fn root(&self) -> Option<[u8; 32]> {
        match self {
            Self::Inherited(image) => image.root(),
            Self::File(_) => None,
        }
    }
    pub(crate) fn inherited(&self) -> Option<&CheckedImage> {
        match self {
            Self::Inherited(image) => Some(image),
            Self::File(_) => None,
        }
    }
    pub(crate) fn store(&self, cache: &Cache, staged: &StagedDirectory) -> anyhow::Result<()> {
        let descriptor = self.descriptor();
        let digest = descriptor.digest.parse()?;
        match self {
            Self::Inherited(_) => staged.copy_blob(cache, &digest, descriptor.size)?,
            Self::File(file) => staged.store_verified(file.reader(), &digest, descriptor.size)?,
        }
        Ok(())
    }
}
