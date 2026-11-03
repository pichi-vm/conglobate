// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{cache::Cache, firmware::FirmwareFile, input::CheckedImage, output::StagedDirectory};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) enum Pmi<'b, 'w> {
    Inherited(&'b CheckedImage),
    File(Arc<FirmwareFile<'w>>),
}
impl Pmi<'_, '_> {
    pub(crate) fn descriptor(&self) -> oci_spec::image::Descriptor {
        match self {
            Self::Inherited(image) => image
                .image()
                .manifest()
                .layers()
                .iter()
                .find(|layer| {
                    layer.media_type().as_ref() == crate::artifact::MEDIA_TYPE_PICHI_PMI_V1
                })
                .cloned()
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
        let digest = descriptor.digest();
        match self {
            Self::Inherited(_) => staged.copy_blob(cache, digest, descriptor.size())?,
            Self::File(file) => staged.store_verified(file.reader(), digest, descriptor.size())?,
        }
        Ok(())
    }
}
