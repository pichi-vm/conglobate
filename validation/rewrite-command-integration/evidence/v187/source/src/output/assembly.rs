// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::StagedDirectory;
use crate::{
    cache::{Cache, CacheError, ResolvedImage, VerifiedChain},
    image::IndexDigest,
};
use pichi_artifact::Digest;

/// A self-contained staged image whose manifest, referenced content and chain
/// were verified from the output storage. No mutable staging authority escapes.
/// This is not a successful recipe execution or an inventory/publication receipt.
#[derive(Debug)]
pub struct AssembledImage {
    staged: StagedDirectory,
    image: ResolvedImage,
    chain: Option<VerifiedChain>,
}
#[derive(Debug, thiserror::Error)]
pub enum AssemblyFailure {
    #[error("output staging: {0}")]
    Before(std::io::Error),
    #[error("output assembly: {source}")]
    Staged {
        source: CacheError,
        staged: StagedDirectory,
    },
}
impl AssemblyFailure {
    pub fn into_parts(self) -> (CacheError, Option<StagedDirectory>) {
        match self {
            Self::Before(source) => (source.into(), None),
            Self::Staged { source, staged } => (source, Some(staged)),
        }
    }
}
impl AssembledImage {
    pub(crate) fn publish_inventory(
        self,
        inventory: &crate::inventory::Inventory,
    ) -> Result<(), crate::pipeline::BuildError> {
        inventory.check_output(&self)?;
        self.staged
            .write_metadata(self.image.index().bytes(), inventory.bytes())?;
        self.staged.publish()?;
        Ok(())
    }
    pub fn image(&self) -> &ResolvedImage {
        &self.image
    }
    pub fn chain(&self) -> Option<&VerifiedChain> {
        self.chain.as_ref()
    }
    pub fn discard(self) -> std::io::Result<()> {
        self.staged.discard()
    }
}
impl AssembledImage {
    /// Preserve this selected base's exact manifest and every referenced blob.
    /// The pipeline must select its output base separately from its tool inputs.
    /// Inline canonical config still gets a blob in the layout.
    pub fn inherit(
        cache: &Cache,
        image: &ResolvedImage,
        destination: &std::path::Path,
    ) -> Result<AssembledImage, AssemblyFailure> {
        let staged = StagedDirectory::create(destination).map_err(AssemblyFailure::Before)?;
        let assembled = (|| -> Result<_, CacheError> {
            let config = &image.manifest().config;
            if config.is_empty() {
                let digest = staged.store_blob(b"{}")?;
                if digest.to_string() != config.digest || config.size != 2 {
                    return Err(CacheError::Invalid("invalid canonical output config"));
                }
            } else {
                let digest: Digest = config
                    .digest
                    .parse()
                    .map_err(|_| CacheError::Invalid("invalid config digest"))?;
                staged.copy_blob(cache, &digest, config.size)?;
            }
            for layer in &image.manifest().layers {
                let digest = layer
                    .digest_str()
                    .parse()
                    .map_err(|_| CacheError::Invalid("invalid output layer digest"))?;
                staged.copy_blob(cache, &digest, layer.size())?;
            }
            // Preserve extensions, annotations and original JSON bytes rather
            // than reserializing a typed subset of the inherited manifest.
            let manifest = image.manifest_bytes();
            let digest = staged.store_blob(manifest.bytes())?;
            if &digest != manifest.digest() {
                return Err(CacheError::Invalid("captured manifest identity mismatch"));
            }
            let index = serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.index.v1+json",
                "manifests": [{
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "artifactType": pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
                    "digest": digest.to_string(),
                    "size": manifest.bytes().len(),
                    "platform": {"os": "pichi", "architecture": image.architecture().oci_name()}
                }]
            }))?;
            let pin: IndexDigest = staged
                .store_blob(&index)?
                .to_string()
                .parse()
                .map_err(|_| CacheError::Invalid("invalid output index digest"))?;
            staged.write_file(&staged.temporary.path().join("index.json"), &index)?;
            staged.write_file(
                &staged.temporary.path().join("oci-layout"),
                b"{\"imageLayoutVersion\":\"1.0.0\"}",
            )?;
            let output_cache = Cache::open(staged.temporary.path())?;
            let output_image = output_cache.resolve(pin)?;
            output_cache.verify_references(&output_image)?;
            let chain = output_cache.verify_chain(&output_image)?;
            // The index is the layout entrypoint, not referenced by its own
            // manifest. Remove the temporary lookup blob after validation.
            std::fs::remove_file(staged.blob_path(&Digest::from_bytes_sha256(&index)))?;
            Ok((output_image, chain))
        })();
        match assembled {
            Ok((image, chain)) => Ok(AssembledImage {
                staged,
                image,
                chain,
            }),
            Err(source) => Err(AssemblyFailure::Staged { source, staged }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::HostArchitecture;
    use std::{collections::BTreeMap, fs};
    #[derive(Debug)]
    struct Input {
        directory: tempfile::TempDir,
        pin: IndexDigest,
        manifest: Vec<u8>,
        blobs: BTreeMap<String, Vec<u8>>,
    }
    impl Input {
        fn new(empty_config: bool) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let mut blobs = BTreeMap::new();
            let pmi = b"fixture firmware bytes".to_vec();
            let dtb = b"fixture dtb bytes".to_vec();
            let config = if empty_config {
                b"{}".to_vec()
            } else {
                b"{\"requirements\":{}}".to_vec()
            };
            for bytes in [&pmi, &dtb, &config] {
                blobs.insert(Digest::from_bytes_sha256(bytes).to_string(), bytes.clone());
            }
            let config_descriptor = if empty_config {
                pichi_artifact::ConfigDescriptor::canonical()
            } else {
                pichi_artifact::ConfigDescriptor::for_config(
                    Digest::from_bytes_sha256(&config).to_string(),
                    config.len() as u64,
                )
            };
            let manifest = serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "artifactType": pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
                "config": config_descriptor,
                "layers": [
                    {"mediaType": "application/vnd.pichi.pmi.v1", "digest": Digest::from_bytes_sha256(&pmi).to_string(), "size": pmi.len()},
                    {"mediaType": "application/vnd.pichi.dtb.v1", "digest": Digest::from_bytes_sha256(&dtb).to_string(), "size": dtb.len()}
                ],
                "annotations": {"example.org/retain": "exactly"},
                "x-preserved-extension": {"nested": [1, 2, 3]}
            })).unwrap();
            let manifest_digest = Digest::from_bytes_sha256(&manifest);
            let index = serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.index.v1+json",
                "manifests": [{
                    "mediaType": "application/vnd.oci.image.manifest.v1+json",
                    "artifactType": pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
                    "digest": manifest_digest.to_string(), "size": manifest.len(),
                    "platform": {"os": "pichi", "architecture": HostArchitecture::current().unwrap().oci_name()}
                }]
            })).unwrap();
            blobs.insert(manifest_digest.to_string(), manifest.clone());
            let index_digest = Digest::from_bytes_sha256(&index);
            fs::create_dir_all(directory.path().join("blobs/sha256")).unwrap();
            for (digest, bytes) in &blobs {
                if empty_config && bytes == b"{}" {
                    continue;
                }
                fs::write(
                    directory
                        .path()
                        .join("blobs/sha256")
                        .join(digest.strip_prefix("sha256:").unwrap()),
                    bytes,
                )
                .unwrap();
            }
            fs::write(
                directory
                    .path()
                    .join("blobs/sha256")
                    .join(index_digest.hex()),
                index,
            )
            .unwrap();
            Self {
                directory,
                pin: index_digest.to_string().parse().unwrap(),
                manifest,
                blobs,
            }
        }
        fn open(&self) -> (Cache, ResolvedImage) {
            let cache = Cache::open(self.directory.path()).unwrap();
            let image = cache.resolve(self.pin).unwrap();
            (cache, image)
        }
    }
    #[test]
    fn inheritance_preserves_all_selected_content_and_exact_manifest_without_source_cache() {
        for empty_config in [true, false] {
            let input = Input::new(empty_config);
            let (cache, image) = input.open();
            let parent = tempfile::tempdir().unwrap();
            let destination = parent.path().join("output");
            let assembled = AssembledImage::inherit(&cache, &image, &destination).unwrap();
            assert!(!destination.exists());
            assert!(assembled.chain().is_none());
            assert_eq!(assembled.image().manifest_bytes().bytes(), input.manifest);
            drop(cache);
            fs::remove_dir_all(input.directory.path()).unwrap();
            let blob_directory = assembled.staged.temporary.path().join("blobs/sha256");
            assert_eq!(
                fs::read_dir(&blob_directory).unwrap().count(),
                input.blobs.len()
            );
            for (digest, expected) in input.blobs {
                assert_eq!(
                    fs::read(blob_directory.join(digest.strip_prefix("sha256:").unwrap())).unwrap(),
                    expected
                );
            }
            assert!(
                !assembled
                    .staged
                    .temporary
                    .path()
                    .join("inventory.cbor")
                    .exists()
            );
            assembled.discard().unwrap();
            assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
        }
    }
    #[test]
    fn missing_reference_retains_only_private_staging_for_cleanup() {
        let input = Input::new(false);
        let (cache, image) = input.open();
        let layer = &image.manifest().layers[1];
        fs::remove_file(
            input
                .directory
                .path()
                .join("blobs/sha256")
                .join(layer.digest_str().strip_prefix("sha256:").unwrap()),
        )
        .unwrap();
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("output");
        let (_, staged) = AssembledImage::inherit(&cache, &image, &destination)
            .unwrap_err()
            .into_parts();
        assert!(!destination.exists());
        staged.unwrap().discard().unwrap();
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
}
