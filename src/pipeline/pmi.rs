// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    artifact::Layer, cache::Cache, firmware::FirmwareFile, input::CheckedImage,
    output::StagedDirectory,
};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) enum Pmi<'b, 'w> {
    Inherited(&'b CheckedImage),
    File(Arc<FirmwareFile<'w>>),
}
impl Pmi<'_, '_> {
    pub(crate) fn layer(&self) -> Layer {
        match self {
            Self::Inherited(image) => image
                .image()
                .manifest()
                .layers
                .iter()
                .find(|layer| matches!(layer, Layer::Pmi(_) | Layer::PmiZstd(_)))
                .expect("selected PMI")
                .clone(),
            Self::File(file) => Layer::Pmi(file.descriptor()),
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
        let layer = self.layer();
        let digest = layer.digest_str().parse()?;
        match self {
            Self::Inherited(_) => staged.copy_blob(cache, &digest, layer.size())?,
            Self::File(file) => staged.store_verified(file.reader(), &digest, layer.size())?,
        }
        Ok(())
    }
}
