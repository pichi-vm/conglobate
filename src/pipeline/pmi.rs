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
    pub(crate) fn source_layer(&self) -> Layer {
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
    pub(crate) fn store(&self, cache: &Cache, staged: &StagedDirectory) -> anyhow::Result<Layer> {
        let layer = self.source_layer();
        let digest = layer.digest_str().parse()?;
        match self {
            Self::Inherited(_) => {
                staged.copy_blob(cache, &digest, layer.size())?;
                Ok(layer)
            }
            Self::File(file) => Ok(Layer::PmiZstd(staged.store_pmi(
                file.reader(),
                &digest,
                layer.size(),
            )?)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        output::AssembledImage, pipeline::output::StageOutput, step::SetupBudget,
        storage::Workspace,
    };
    use sha2::{Digest as _, Sha256};
    use std::{fs, io::Read as _};

    #[test]
    #[ignore = "requires root for private RAM workspace"]
    fn captured_pmi_assembly_emits_compressed_descriptor_and_original_content() {
        let directory = tempfile::tempdir().unwrap();
        let original = b"opaque captured PMI; exact original length";
        let path = directory.path().join("captured");
        fs::write(&path, original).unwrap();
        let workspace = Workspace::create().unwrap();
        let captured = FirmwareFile::capture(
            fs::File::open(&path).unwrap(),
            &workspace,
            &SetupBudget::begin(),
        )
        .unwrap();
        let selection = StageOutput {
            root: None,
            pmi: Some(Pmi::File(Arc::new(captured))),
            bootconfig: None,
        };
        let cache = Cache::open(directory.path()).unwrap();
        let destination = directory.path().join("output");
        let assembled = AssembledImage::components(&cache, &selection, None, &destination).unwrap();
        let layers = &assembled.image().manifest().layers;
        assert_eq!(layers.len(), 1);
        let Layer::PmiZstd(descriptor) = &layers[0] else {
            panic!("new PMI must use compressed transport")
        };
        let descriptor = descriptor.clone();
        assembled.publish().unwrap();
        let digest: crate::artifact::Digest = descriptor.digest.parse().unwrap();
        let encoded = fs::read(destination.join("blobs/sha256").join(digest.hex())).unwrap();
        assert_eq!(descriptor.size, encoded.len() as u64);
        assert_eq!(
            digest,
            crate::artifact::Digest::Sha256(Sha256::digest(&encoded).into())
        );
        let mut decoded = Vec::new();
        ruzstd::decoding::StreamingDecoder::new(encoded.as_slice())
            .unwrap()
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded, original);
    }
}
