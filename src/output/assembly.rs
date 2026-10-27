// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{StagedDirectory, TRANSITIONAL_PMI_NAME, transitional_manifest};
use crate::artifact::Digest;
use crate::{
    cache::{Cache, CacheError, ResolvedImage, VerifiedChain},
    image::IndexDigest,
};

/// Stage delta scutes and optional firmware beside the OCI layout.
///
/// `scutes` is the produced chain only, base first. Inherited layers stay in
/// the OCI manifest; the host prepends that source carapace when it reads
/// `build.yaml`. An empty slice still writes `build.yaml` with `scutes: []`.
fn stage_delta_output(
    staged: &StagedDirectory,
    scutes: &[&crate::storage::SealedScute<'_>],
    pmi: Option<&crate::firmware::FinalizedFirmware<'_>>,
) -> Result<(), crate::cache::CacheError> {
    let salts: Vec<&str> = scutes
        .iter()
        .map(|scute| scute.descriptor().annotations.salt.as_str())
        .collect();
    let manifest = transitional_manifest(&salts, pmi.map(|_| TRANSITIONAL_PMI_NAME.to_string()))?;
    if manifest.scutes.len() != scutes.len() {
        return Err(crate::cache::CacheError::Invalid(
            "delta scute manifest length mismatch",
        ));
    }
    for (entry, scute) in manifest.scutes.iter().zip(scutes) {
        staged.write_exact_file(&entry.cow, scute.data_reader(), scute.descriptor().size)?;
        let verity_len = scute.verity_len()?;
        staged.write_exact_file(&entry.verity, scute.verity_reader()?, verity_len)?;
    }
    if let Some(pair) = pmi {
        let descriptor = pair.pmi_descriptor();
        staged.write_exact_file(TRANSITIONAL_PMI_NAME, pair.pmi_reader(), descriptor.size)?;
    }
    staged.write_build_yaml(&manifest)?;
    Ok(())
}

/// A self-contained staged image whose manifest, referenced content and chain
/// were verified from the output storage. No mutable staging authority escapes.
/// This is not a successful recipe execution or an inventory/publication receipt.
#[derive(Debug)]
pub(crate) struct AssembledImage {
    staged: StagedDirectory,
    image: ResolvedImage,
    chain: Option<VerifiedChain>,
}
impl AssembledImage {
    fn verity_annotations() -> serde_json::Value {
        serde_json::json!({
            "dev.pichi.carapace.verity.algo": "sha256",
            "dev.pichi.carapace.verity.data-block-size": "4096",
            "dev.pichi.carapace.verity.hash-block-size": "4096",
            "dev.pichi.carapace.verity.version": "1",
            "dev.pichi.carapace.verity.hash-type": "1"
        })
    }
    pub(crate) fn publish_inventory(
        self,
        inventory: &crate::inventory::Inventory,
    ) -> anyhow::Result<()> {
        inventory.check_output(&self)?;
        self.staged.write_inventory(inventory.bytes())?;
        self.staged.publish()?;
        Ok(())
    }
    pub(crate) fn image(&self) -> &ResolvedImage {
        &self.image
    }
    pub(crate) fn chain(&self) -> Option<&VerifiedChain> {
        self.chain.as_ref()
    }
    #[cfg(test)]
    pub(crate) fn discard(self) -> std::io::Result<()> {
        self.staged.discard()
    }
}
impl AssembledImage {
    pub(crate) fn derived(
        cache: &Cache,
        image: &ResolvedImage,
        scutes: &[std::sync::Arc<crate::storage::SealedScute<'_>>],
        destination: &std::path::Path,
        dtb: &crate::transform::TransformedDtb,
    ) -> anyhow::Result<Self> {
        Self::assemble(
            cache,
            image,
            destination,
            dtb.bytes(),
            &scutes.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
        )
    }
    /// Replace the firmware pair while preserving only the chosen output base.
    /// Tool image components are never implicit output ancestry.
    pub(crate) fn firmware(
        cache: &Cache,
        base: Option<&ResolvedImage>,
        pair: &crate::firmware::FinalizedFirmware<'_>,
        scutes: &[std::sync::Arc<crate::storage::SealedScute<'_>>],
        destination: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let staged = StagedDirectory::create(destination)?;
        let result = (|| -> Result<_, CacheError> {
            let mut manifest = if let Some(base) = base {
                let config = &base.manifest().config;
                if config.is_empty() {
                    staged.store_blob(b"{}")?;
                } else {
                    let digest = config
                        .digest
                        .parse()
                        .map_err(|_| CacheError::Invalid("invalid config digest"))?;
                    staged.copy_blob(cache, &digest, config.size)?;
                }
                let mut document: serde_json::Value =
                    serde_json::from_slice(base.manifest_bytes().bytes())?;
                let mut layers = Vec::new();
                for (index, layer) in base.manifest().layers.iter().enumerate() {
                    if matches!(
                        layer,
                        crate::artifact::Layer::Pmi(_) | crate::artifact::Layer::Dtb(_)
                    ) {
                        continue;
                    }
                    let digest = layer
                        .digest_str()
                        .parse()
                        .map_err(|_| CacheError::Invalid("invalid layer digest"))?;
                    staged.copy_blob(cache, &digest, layer.size())?;
                    layers.push(document["layers"][index].clone());
                }
                document["layers"] = layers.into();
                document
            } else {
                staged.store_blob(b"{}")?;
                serde_json::json!({"schemaVersion":2,"mediaType":"application/vnd.oci.image.manifest.v1+json",
                    "artifactType":crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
                    "config":crate::artifact::ConfigDescriptor::canonical(),"layers":[]})
            };
            let mut parent = match base {
                Some(image) => image.manifest().carapace_verity_hash(),
                None => Some([0; 32]),
            };
            if base.is_none() && !scutes.is_empty() {
                manifest["annotations"] = Self::verity_annotations();
            }
            for scute in scutes {
                if parent.as_ref() != Some(scute.parent()) {
                    return Err(CacheError::Invalid(
                        "produced scute parent differs from firmware output base",
                    ));
                }
                staged.store_scute(scute)?;
                manifest["layers"]
                    .as_array_mut()
                    .ok_or(CacheError::Invalid("missing layers"))?
                    .push(serde_json::to_value(crate::artifact::Layer::Scute(
                        scute.descriptor().clone(),
                    ))?);
                parent = Some(*scute.root());
                manifest["annotations"]["dev.pichi.carapace.verity.hash"] =
                    hex::encode(scute.root()).into();
            }
            let pmi = pair.pmi_descriptor();
            let digest = pmi
                .digest
                .parse()
                .map_err(|_| CacheError::Invalid("invalid captured PMI digest"))?;
            staged.store_verified(pair.pmi_reader(), &digest, pmi.size)?;
            let bytes = pair
                .dtb()
                .bytes()
                .ok_or(CacheError::Invalid("missing finalized DTB"))?;
            let dtb = crate::artifact::DtbDescriptor {
                digest: staged.store_blob(bytes)?.to_string(),
                size: bytes.len() as u64,
            };
            let layers = manifest["layers"]
                .as_array_mut()
                .ok_or(CacheError::Invalid("missing output layers"))?;
            layers.push(serde_json::to_value(crate::artifact::Layer::Pmi(pmi))?);
            layers.push(serde_json::to_value(crate::artifact::Layer::Dtb(dtb))?);
            let resolved = Self::resolve_staging(
                &staged,
                &serde_json::to_vec(&manifest)?,
                crate::cache::HostArchitecture::current()?,
            )?;
            let produced: Vec<_> = scutes.iter().map(AsRef::as_ref).collect();
            stage_delta_output(&staged, &produced, Some(pair))?;
            Ok(resolved)
        })();
        match result {
            Ok((image, chain)) => Ok(Self {
                staged,
                image,
                chain,
            }),
            Err(source) => {
                let error = anyhow::Error::new(source);
                Err(match staged.discard() {
                    Ok(()) => error,
                    Err(cleanup) => error.context(format!("output staging cleanup: {cleanup}")),
                })
            }
        }
    }
    pub(crate) fn bootstrap_layers(
        scutes: &[std::sync::Arc<crate::storage::SealedScute<'_>>],
        destination: &std::path::Path,
    ) -> anyhow::Result<Self> {
        Self::assemble_bootstrap(
            &scutes.iter().map(AsRef::as_ref).collect::<Vec<_>>(),
            destination,
        )
    }
    fn assemble_bootstrap(
        scutes: &[&crate::storage::SealedScute<'_>],
        destination: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let staged = StagedDirectory::create(destination)?;
        let result = (|| -> Result<_, CacheError> {
            if scutes.is_empty() {
                return Err(CacheError::Invalid("missing bootstrap root scute"));
            }
            if scutes[0].parent() != &[0; 32] {
                return Err(CacheError::Invalid("bootstrap scute has a nonzero parent"));
            }
            staged.store_blob(b"{}")?;
            let mut parent = [0; 32];
            let mut layers = Vec::with_capacity(scutes.len());
            for scute in scutes {
                if scute.parent() != &parent {
                    return Err(CacheError::Invalid("bootstrap scute parent mismatch"));
                }
                staged.store_scute(scute)?;
                layers.push(crate::artifact::Layer::Scute(scute.descriptor().clone()));
                parent = *scute.root();
            }
            let mut annotations = Self::verity_annotations();
            annotations["dev.pichi.carapace.verity.hash"] = hex::encode(parent).into();
            let manifest = serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "artifactType": crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
                "config": crate::artifact::ConfigDescriptor::canonical(),
                "layers": layers,
                "annotations": annotations
            }))?;
            let resolved = Self::resolve_staging(
                &staged,
                &manifest,
                crate::cache::HostArchitecture::current()?,
            )?;
            stage_delta_output(&staged, scutes, None)?;
            Ok(resolved)
        })();
        match result {
            Ok((image, chain)) => Ok(Self {
                staged,
                image,
                chain,
            }),
            Err(source) => {
                let error = anyhow::Error::new(source);
                Err(match staged.discard() {
                    Ok(()) => error,
                    Err(cleanup) => error.context(format!("output staging cleanup: {cleanup}")),
                })
            }
        }
    }
    fn resolve_staging(
        staged: &StagedDirectory,
        manifest: &[u8],
        architecture: crate::cache::HostArchitecture,
    ) -> Result<(ResolvedImage, Option<VerifiedChain>), CacheError> {
        let digest = staged.store_blob(manifest)?;
        let index = serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 2,
            "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [{
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "artifactType": crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
                "digest": digest.to_string(),
                "size": manifest.len(),
                "platform": {"os": "pichi", "architecture": architecture.oci_name()}
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
        let chain = output_cache.verify_content(&output_image)?;
        // The index is the layout entrypoint, not referenced by its own
        // manifest. Remove the temporary lookup blob after validation.
        std::fs::remove_file(staged.blob_path(&Digest::from_bytes_sha256(&index)))?;
        Ok((output_image, chain))
    }
    fn assemble(
        cache: &Cache,
        image: &ResolvedImage,
        destination: &std::path::Path,
        dtb: Option<&[u8]>,
        scutes: &[&crate::storage::SealedScute<'_>],
    ) -> anyhow::Result<Self> {
        let staged = StagedDirectory::create(destination)?;
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
            let mut manifest: serde_json::Value =
                serde_json::from_slice(image.manifest_bytes().bytes())?;
            let mut changed = false;
            for (position, layer) in image.manifest().layers.iter().enumerate() {
                if let (crate::artifact::Layer::Dtb(old), Some(bytes)) = (layer, dtb) {
                    let digest = staged.store_blob(bytes)?;
                    if digest.to_string() != old.digest || bytes.len() as u64 != old.size {
                        let pmi = image
                            .manifest()
                            .layers
                            .iter()
                            .find_map(|layer| match layer {
                                crate::artifact::Layer::Pmi(descriptor) => Some(descriptor),
                                _ => None,
                            })
                            .ok_or(CacheError::Invalid("changed DTB has no firmware contract"))?;
                        let firmware_digest = pmi
                            .digest
                            .parse()
                            .map_err(|_| CacheError::Invalid("invalid PMI digest"))?;
                        cache
                            .firmware_contract(&firmware_digest, pmi.size)?
                            .check_size(bytes.len())?;
                        let descriptor = &mut manifest["layers"][position];
                        descriptor["digest"] = digest.to_string().into();
                        descriptor["size"] = (bytes.len() as u64).into();
                        // Inline OCI data must describe the replaced bytes too.
                        if let Some(object) = descriptor.as_object_mut() {
                            object.remove("data");
                            object.remove("urls");
                        }
                        changed = true;
                    }
                    continue;
                }
                let digest = layer
                    .digest_str()
                    .parse()
                    .map_err(|_| CacheError::Invalid("invalid output layer digest"))?;
                staged.copy_blob(cache, &digest, layer.size())?;
            }
            let mut parent = image.manifest().carapace_verity_hash();
            for scute in scutes {
                if parent.as_ref() != Some(scute.parent()) {
                    return Err(CacheError::Invalid(
                        "produced scute parent differs from output base",
                    ));
                }
                staged.store_scute(scute)?;
                manifest["layers"]
                    .as_array_mut()
                    .ok_or(CacheError::Invalid("missing manifest layers"))?
                    .push(serde_json::to_value(crate::artifact::Layer::Scute(
                        scute.descriptor().clone(),
                    ))?);
                manifest["annotations"]["dev.pichi.carapace.verity.hash"] =
                    hex::encode(scute.root()).into();
                parent = Some(*scute.root());
                changed = true;
            }
            // Preserve unrelated fields, and preserve exact JSON bytes for no-op edits.
            let manifest = if changed {
                serde_json::to_vec(&manifest)?
            } else {
                image.manifest_bytes().bytes().to_vec()
            };
            let resolved = Self::resolve_staging(&staged, &manifest, image.architecture())?;
            stage_delta_output(&staged, scutes, None)?;
            Ok(resolved)
        })();
        match assembled {
            Ok((image, chain)) => Ok(AssembledImage {
                staged,
                image,
                chain,
            }),
            Err(source) => {
                let error = anyhow::Error::new(source);
                Err(match staged.discard() {
                    Ok(()) => error,
                    Err(cleanup) => error.context(format!("output staging cleanup: {cleanup}")),
                })
            }
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
            Self::with_firmware(
                empty_config,
                crate::firmware::tests::Fixture::image(
                    ".pmi.vm",
                    &crate::firmware::tests::Fixture::detached(),
                    4096,
                ),
            )
        }
        fn with_firmware(empty_config: bool, pmi: Vec<u8>) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let mut blobs = BTreeMap::new();
            let dtb =
                devtree::OwnedTree::new(devtree::OwnedNode::new("").with_child(
                    devtree::OwnedNode::new("chosen").with_property(
                        devtree::OwnedProperty::new("bootargs").with_str("corium=old"),
                    ),
                ))
                .encode()
                .unwrap();
            let config = if empty_config {
                b"{}".to_vec()
            } else {
                b"{\"requirements\":{}}".to_vec()
            };
            for bytes in [&pmi, &dtb, &config] {
                blobs.insert(Digest::from_bytes_sha256(bytes).to_string(), bytes.clone());
            }
            let config_descriptor = if empty_config {
                crate::artifact::ConfigDescriptor::canonical()
            } else {
                crate::artifact::ConfigDescriptor::for_config(
                    Digest::from_bytes_sha256(&config).to_string(),
                    config.len() as u64,
                )
            };
            let manifest = serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "artifactType": crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
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
                    "artifactType": crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
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
    fn opaque_firmware_allows_unchanged_dtb_but_rejects_edits() {
        let input = Input::with_firmware(true, b"opaque firmware".to_vec());
        let (cache, image) = input.open();
        let old: serde_json::Value = serde_json::from_slice(&input.manifest).unwrap();
        let bytes = &input.blobs[old["layers"][1]["digest"].as_str().unwrap()];
        let parent = tempfile::tempdir().unwrap();
        let unchanged = crate::recipe::TransformSelection::Authored(vec![])
            .apply(Some(bytes), None)
            .unwrap();
        AssembledImage::derived(
            &cache,
            &image,
            &[],
            &parent.path().join("unchanged"),
            &unchanged,
        )
        .unwrap()
        .discard()
        .unwrap();
        let changed = crate::recipe::TransformSelection::Builtin
            .apply(Some(bytes), Some("new"))
            .unwrap();
        let failure = AssembledImage::derived(
            &cache,
            &image,
            &[],
            &parent.path().join("changed"),
            &changed,
        )
        .unwrap_err();
        assert!(failure.to_string().contains("firmware/DTB compatibility"));
        assert!(!parent.path().join("changed").exists());
    }
    #[test]
    fn transformed_output_replaces_only_dtb_and_is_self_contained() {
        let input = Input::new(false);
        let (cache, image) = input.open();
        let old: serde_json::Value = serde_json::from_slice(&input.manifest).unwrap();
        let old_digest = old["layers"][1]["digest"].as_str().unwrap();
        let result = crate::recipe::TransformSelection::Builtin
            .apply(Some(&input.blobs[old_digest]), Some("new"))
            .unwrap();
        let parent = tempfile::tempdir().unwrap();
        let assembled =
            AssembledImage::derived(&cache, &image, &[], &parent.path().join("output"), &result)
                .unwrap();
        let new: serde_json::Value =
            serde_json::from_slice(assembled.image().manifest_bytes().bytes()).unwrap();
        assert_eq!(new["config"], old["config"]);
        assert_eq!(new["layers"][0], old["layers"][0]);
        assert_eq!(new["annotations"], old["annotations"]);
        assert_eq!(new["x-preserved-extension"], old["x-preserved-extension"]);
        assert_ne!(new["layers"][1]["digest"], old["layers"][1]["digest"]);
        let digest = Digest::from_bytes_sha256(result.bytes().unwrap());
        assert_eq!(new["layers"][1]["digest"], digest.to_string());
        assert_eq!(
            fs::read(assembled.staged.blob_path(&digest)).unwrap(),
            result.bytes().unwrap()
        );
        assert!(
            !assembled
                .staged
                .blob_path(&old_digest.parse().unwrap())
                .exists()
        );
        drop(cache);
        fs::remove_dir_all(input.directory.path()).unwrap();
        let cache = Cache::open(assembled.staged.temporary.path()).unwrap();
        cache.verify_references(assembled.image()).unwrap();
        assembled.discard().unwrap();
    }
    #[test]
    fn inheritance_preserves_all_selected_content_and_exact_manifest_without_source_cache() {
        for empty_config in [true, false] {
            let input = Input::new(empty_config);
            let (cache, image) = input.open();
            let parent = tempfile::tempdir().unwrap();
            let destination = parent.path().join("output");
            let unchanged = crate::recipe::TransformSelection::Authored(vec![])
                .apply(
                    Some(&input.blobs[image.manifest().layers[1].digest_str()]),
                    None,
                )
                .unwrap();
            let assembled =
                AssembledImage::derived(&cache, &image, &[], &destination, &unchanged).unwrap();
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
    fn derived_output_stages_empty_delta_build_yaml_beside_oci() {
        let input = Input::new(true);
        let (cache, image) = input.open();
        let unchanged = crate::recipe::TransformSelection::Authored(vec![])
            .apply(
                Some(&input.blobs[image.manifest().layers[1].digest_str()]),
                None,
            )
            .unwrap();
        let parent = tempfile::tempdir().unwrap();
        let assembled = AssembledImage::derived(
            &cache,
            &image,
            &[],
            &parent.path().join("output"),
            &unchanged,
        )
        .unwrap();
        let root = assembled.staged.temporary.path();
        assert!(root.join("oci-layout").is_file());
        assert!(root.join("index.json").is_file());
        assert!(root.join("blobs").is_dir());
        let yaml = fs::read_to_string(root.join("build.yaml")).unwrap();
        let parsed = crate::recipe_io::BuildOutput::parse(&yaml).unwrap();
        assert!(parsed.scutes.is_empty(), "{yaml}");
        assert!(parsed.pmi.is_none(), "{yaml}");
        assert!(!yaml.contains("pmi"), "{yaml}");
        assert!(!root.join("0000.cow").exists());
        assert!(!root.join("boot.pmi").exists());
        assembled.discard().unwrap();
    }
    #[test]
    fn captured_unchanged_dtb_survives_loss_of_cache_copy() {
        let input = Input::new(false);
        let (cache, image) = input.open();
        let digest = image.manifest().layers[1].digest_str();
        let unchanged = crate::recipe::TransformSelection::Authored(vec![])
            .apply(Some(&input.blobs[digest]), None)
            .unwrap();
        fs::remove_file(
            input
                .directory
                .path()
                .join("blobs/sha256")
                .join(digest.strip_prefix("sha256:").unwrap()),
        )
        .unwrap();
        let parent = tempfile::tempdir().unwrap();
        let assembled = AssembledImage::derived(
            &cache,
            &image,
            &[],
            &parent.path().join("output"),
            &unchanged,
        )
        .unwrap();
        assert_eq!(assembled.image().manifest_bytes().bytes(), input.manifest);
        Cache::open(assembled.staged.temporary.path())
            .unwrap()
            .verify_references(assembled.image())
            .unwrap();
        assembled.discard().unwrap();
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
    #[test]
    fn missing_reference_retains_only_private_staging_for_cleanup() {
        let input = Input::new(false);
        let (cache, image) = input.open();
        let layer = &image.manifest().layers[0];
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
        let unchanged = crate::recipe::TransformSelection::Authored(vec![])
            .apply(
                Some(&input.blobs[image.manifest().layers[1].digest_str()]),
                None,
            )
            .unwrap();
        let _error =
            AssembledImage::derived(&cache, &image, &[], &destination, &unchanged).unwrap_err();
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
}
