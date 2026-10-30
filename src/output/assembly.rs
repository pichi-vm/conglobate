// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::StagedDirectory;
use crate::artifact::{CARAPACE_ROOT_ANNOTATION, Digest};
use crate::{
    cache::{Cache, CacheError, ResolvedImage, VerifiedChain},
    image::IndexDigest,
};
use sha2::{Digest as _, Sha256};

/// A self-contained staged image whose manifest, referenced content and chain
/// were verified from the output storage. No mutable staging authority escapes.
/// This is not a successful plan execution or a publication receipt.
#[derive(Debug)]
pub(crate) struct AssembledImage {
    staged: StagedDirectory,
    image: ResolvedImage,
    chain: Option<VerifiedChain>,
}
impl AssembledImage {
    fn verity_annotations() -> serde_json::Value {
        // Reconstruction parameters are shared by every scute in this output.
        serde_json::json!({
            "dev.pichi.carapace.verity.algo": "sha256",
            "dev.pichi.carapace.verity.data-block-size": "4096",
            "dev.pichi.carapace.verity.hash-block-size": "4096",
            "dev.pichi.carapace.verity.version": "1",
            "dev.pichi.carapace.verity.hash-type": "1"
        })
    }
    fn set_tip_annotation(manifest: &mut serde_json::Value, root: Option<[u8; 32]>) {
        if let Some(root) = root {
            manifest["annotations"][CARAPACE_ROOT_ANNOTATION] = hex::encode(root).into();
        }
    }
    /// Stamp the resolved input before hashing and validating the final layout.
    pub(crate) fn bind_build(
        mut self,
        request: &crate::request::BuildRequest,
        build: &crate::input::CapturedBuild,
    ) -> anyhow::Result<Self> {
        let mut manifest: serde_json::Value =
            serde_json::from_slice(self.image.manifest_bytes().bytes())?;
        let annotations = manifest
            .as_object_mut()
            .expect("validated manifest")
            .entry("annotations")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("invalid manifest annotations"))?;
        // Inherited provenance belongs to the old build. Keep unrelated and
        // verity annotations, but replace the entire build binding.
        annotations.retain(|key, _| !key.starts_with("dev.pichi.build."));
        let commit = build.context().commit();
        let algorithm = match commit.kind() {
            gix::hash::Kind::Sha1 => "sha1",
            gix::hash::Kind::Sha256 => "sha256",
            _ => anyhow::bail!("unsupported Git commit algorithm"),
        };
        let context: &std::path::Path = request.context().as_ref();
        for (key, value) in [
            ("commit", format!("{algorithm}:{commit}")),
            (
                "context",
                if context.as_os_str().is_empty() {
                    ".".into()
                } else {
                    context
                        .to_str()
                        .ok_or_else(|| anyhow::anyhow!("context is not UTF-8"))?
                        .into()
                },
            ),
            (
                "plan",
                build
                    .plan_document()
                    .path()
                    .as_ref()
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("plan path is not UTF-8"))?
                    .into(),
            ),
            (
                "lock",
                build
                    .lock_document()
                    .path()
                    .as_ref()
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("lock path is not UTF-8"))?
                    .into(),
            ),
        ] {
            annotations.insert(format!("dev.pichi.build.{key}"), value.into());
        }
        if let Some(location) = request.location() {
            annotations.insert("dev.pichi.build.location".into(), location.into());
        }
        let old_digest = self.image.manifest_bytes().digest().clone();
        let (image, chain) = Self::resolve_staging(
            &self.staged,
            &serde_json::to_vec(&manifest)?,
            self.image.architecture(),
        )?;
        anyhow::ensure!(
            self.chain.as_ref().map(|chain| chain.root())
                == chain.as_ref().map(|chain| chain.root()),
            "build binding changed the scute graph"
        );
        let new_digest = Digest::Sha256(Sha256::digest(image.manifest_bytes().bytes()).into());
        if old_digest != new_digest {
            std::fs::remove_file(self.staged.blob_path(&old_digest))?;
        }
        self.image = image;
        self.chain = chain;
        Ok(self)
    }
    pub(crate) fn publish(self) -> std::io::Result<()> {
        self.staged.publish()
    }
    #[cfg(test)]
    pub(crate) fn image(&self) -> &ResolvedImage {
        &self.image
    }
    #[cfg(test)]
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
                Some(image) => cache.verify_chain(image)?.map(|chain| *chain.root()),
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
            }
            Self::set_tip_annotation(&mut manifest, parent.filter(|root| root != &[0; 32]));
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
            annotations[CARAPACE_ROOT_ANNOTATION] = hex::encode(parent).into();
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
        let index = serde_json::to_vec(&crate::artifact::Index {
            manifests: vec![crate::artifact::IndexDescriptor {
                media_type: crate::artifact::MEDIA_TYPE_OCI_MANIFEST_V1.into(),
                artifact_type: Some(crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1.into()),
                digest: digest.to_string(),
                size: manifest.len() as u64,
                platform: Some(crate::artifact::Platform {
                    os: "pichi".into(),
                    architecture: architecture.oci_name().into(),
                    variant: None,
                }),
                annotations: Default::default(),
            }],
            ..Default::default()
        })?;
        let pin: IndexDigest = staged
            .store_blob(&index)?
            .to_string()
            .parse()
            .map_err(|_| CacheError::Invalid("invalid output index digest"))?;
        staged.write_file(&staged.temporary.path().join("index.json"), &index)?;
        staged.write_file(
            &staged.temporary.path().join("oci-layout"),
            &serde_json::to_vec(&crate::artifact::LayoutMarker::default())?,
        )?;
        let output_cache = Cache::open(staged.temporary.path())?;
        let output_image = output_cache.resolve(pin)?;
        let chain = output_cache.verify_content(&output_image)?;
        // The index is the layout entrypoint, not referenced by its own
        // manifest. Remove the temporary lookup blob after validation.
        std::fs::remove_file(staged.blob_path(&Digest::Sha256(Sha256::digest(&index).into())))?;
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
            let mut parent = cache.verify_chain(image)?.map(|chain| *chain.root());
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
                parent = Some(*scute.root());
                changed = true;
            }
            // Preserve unrelated fields, and preserve exact JSON bytes for no-op edits.
            // A changed chain carries the final produced root.
            let manifest = if changed {
                Self::set_tip_annotation(&mut manifest, parent);
                serde_json::to_vec(&manifest)?
            } else {
                image.manifest_bytes().bytes().to_vec()
            };
            let resolved = Self::resolve_staging(&staged, &manifest, image.architecture())?;
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
                blobs.insert(
                    Digest::Sha256(Sha256::digest(bytes).into()).to_string(),
                    bytes.clone(),
                );
            }
            let config_descriptor = if empty_config {
                crate::artifact::ConfigDescriptor::canonical()
            } else {
                crate::artifact::ConfigDescriptor::for_config(
                    Digest::Sha256(Sha256::digest(&config).into()).to_string(),
                    config.len() as u64,
                )
            };
            let manifest = serde_json::to_vec_pretty(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "artifactType": crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
                "config": config_descriptor,
                "layers": [
                    {"mediaType": "application/vnd.pichi.pmi.v1", "digest": Digest::Sha256(Sha256::digest(&pmi).into()).to_string(), "size": pmi.len()},
                    {"mediaType": "application/vnd.pichi.dtb.v1", "digest": Digest::Sha256(Sha256::digest(&dtb).into()).to_string(), "size": dtb.len()}
                ],
                "annotations": {"example.org/retain": "exactly"},
                "x-preserved-extension": {"nested": [1, 2, 3]}
            })).unwrap();
            let manifest_digest = Digest::Sha256(Sha256::digest(&manifest).into());
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
            let index_digest = Digest::Sha256(Sha256::digest(&index).into());
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
    fn assert_oci_only(root: &std::path::Path) {
        let names: std::collections::BTreeSet<_> = fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            names,
            ["blobs", "index.json", "oci-layout"]
                .map(std::ffi::OsString::from)
                .into()
        );
    }

    #[test]
    fn publication_binds_committed_inputs_and_replaces_inherited_provenance() {
        use crate::{input::CapturedBuild, request::BuildRequest};
        use std::process::Command;
        for algorithm in ["sha1", "sha256"] {
            for context in [".", "nested/context"] {
                for location in [None, Some("https://example.invalid/source")] {
                    let input = Input::new(false);
                    let (cache, image) = input.open();
                    let repo = tempfile::tempdir().unwrap();
                    let git = |args: &[&str]| {
                        let output = Command::new("git")
                            .arg("-C")
                            .arg(repo.path())
                            .args(args)
                            .output()
                            .unwrap();
                        assert!(
                            output.status.success(),
                            "{}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                        String::from_utf8(output.stdout).unwrap().trim().to_owned()
                    };
                    git(&["init", "-q", &format!("--object-format={algorithm}")]);
                    let selected = repo.path().join(context).join("config");
                    fs::create_dir_all(&selected).unwrap();
                    fs::write(
                        selected.join("plan.yaml"),
                        "image: base\ntasks: {base: {kind: extend, from: registry.test/base:1}}\n",
                    )
                    .unwrap();
                    fs::write(
                        selected.join("lock.yaml"),
                        format!(
                            "version: 1\nimages: {{registry.test/base:1: {{index: sha256:{}}}}}\n",
                            "00".repeat(32)
                        ),
                    )
                    .unwrap();
                    git(&["add", "."]);
                    git(&[
                        "-c",
                        "user.name=Fixture",
                        "-c",
                        "user.email=fixture@example.invalid",
                        "commit",
                        "-qm",
                        "Pin inputs",
                    ]);
                    let commit = git(&["rev-parse", "HEAD"]);
                    let output = tempfile::tempdir().unwrap();
                    let destination = output.path().join("result");
                    let request = BuildRequest::parse_from([
                        "conglobate",
                        "build",
                        "--repository",
                        repo.path().to_str().unwrap(),
                        "--commit",
                        &commit,
                        "--context",
                        context,
                        "--plan",
                        "config/plan.yaml",
                        "--lock",
                        "config/lock.yaml",
                        "--cache",
                        input.directory.path().to_str().unwrap(),
                        "--output",
                        destination.to_str().unwrap(),
                    ])
                    .unwrap()
                    .with_location(location.map(str::to_owned));
                    let captured = CapturedBuild::try_from(&request).unwrap();
                    fs::write(selected.join("plan.yaml"), "working tree changed").unwrap();
                    let unchanged = crate::plan::TransformSelection::Authored(vec![])
                        .apply(
                            Some(&input.blobs[image.manifest().layers[1].digest_str()]),
                            None,
                        )
                        .unwrap();
                    let mut assembled =
                        AssembledImage::derived(&cache, &image, &[], &destination, &unchanged)
                            .unwrap();
                    // Seed stale build keys to exercise pass-through rebinding.
                    let mut manifest: serde_json::Value =
                        serde_json::from_slice(assembled.image().manifest_bytes().bytes()).unwrap();
                    for key in [
                        "commit",
                        "context",
                        "plan",
                        "lock",
                        "location",
                        "tree",
                        "object-format",
                    ] {
                        manifest["annotations"][format!("dev.pichi.build.{key}")] = "stale".into();
                    }
                    let (stale, chain) = AssembledImage::resolve_staging(
                        &assembled.staged,
                        &serde_json::to_vec(&manifest).unwrap(),
                        image.architecture(),
                    )
                    .unwrap();
                    let original = Digest::Sha256(
                        Sha256::digest(assembled.image.manifest_bytes().bytes()).into(),
                    );
                    fs::remove_file(assembled.staged.blob_path(&original)).unwrap();
                    assembled.image = stale;
                    assembled.chain = chain;
                    let assembled = assembled.bind_build(&request, &captured).unwrap();
                    let final_manifest: serde_json::Value =
                        serde_json::from_slice(assembled.image().manifest_bytes().bytes()).unwrap();
                    let annotations = final_manifest["annotations"].as_object().unwrap();
                    assert_eq!(
                        annotations["dev.pichi.build.commit"],
                        format!("{algorithm}:{commit}")
                    );
                    assert_eq!(annotations["dev.pichi.build.context"], context);
                    assert_eq!(annotations["dev.pichi.build.plan"], "config/plan.yaml");
                    assert_eq!(annotations["dev.pichi.build.lock"], "config/lock.yaml");
                    assert_eq!(
                        annotations
                            .get("dev.pichi.build.location")
                            .and_then(|v| v.as_str()),
                        location
                    );
                    assert_eq!(
                        annotations
                            .keys()
                            .filter(|key| key.starts_with("dev.pichi.build."))
                            .count(),
                        if location.is_some() { 5 } else { 4 }
                    );
                    assert!(!annotations.contains_key("dev.pichi.carapace.verity.hash"));
                    assert_eq!(final_manifest["layers"], manifest["layers"]);
                    assert_eq!(final_manifest["config"], manifest["config"]);
                    assert_eq!(
                        annotations["example.org/retain"],
                        manifest["annotations"]["example.org/retain"]
                    );
                    assembled.publish().unwrap();
                    assert!(!destination.join("inventory.cbor").exists());
                    assert_oci_only(&destination);
                    let index: serde_json::Value =
                        serde_json::from_slice(&fs::read(destination.join("index.json")).unwrap())
                            .unwrap();
                    let raw = serde_json::to_vec(&final_manifest).unwrap();
                    assert_eq!(
                        index["manifests"][0]["digest"],
                        Digest::Sha256(Sha256::digest(&raw).into()).to_string()
                    );
                    assert_eq!(index["manifests"][0]["size"], raw.len());
                }
            }
        }
    }
    #[test]
    fn opaque_firmware_allows_unchanged_dtb_but_rejects_edits() {
        let input = Input::with_firmware(true, b"opaque firmware".to_vec());
        let (cache, image) = input.open();
        let old: serde_json::Value = serde_json::from_slice(&input.manifest).unwrap();
        let bytes = &input.blobs[old["layers"][1]["digest"].as_str().unwrap()];
        let parent = tempfile::tempdir().unwrap();
        let unchanged = crate::plan::TransformSelection::Authored(vec![])
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
        let changed = crate::plan::TransformSelection::Builtin
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
        let result = crate::plan::TransformSelection::Builtin
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
        let digest = Digest::Sha256(Sha256::digest(result.bytes().unwrap()).into());
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
            let unchanged = crate::plan::TransformSelection::Authored(vec![])
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
    fn derived_output_publishes_only_oci_to_new_directory_or_empty_mount() {
        let input = Input::new(true);
        let (cache, image) = input.open();
        let unchanged = crate::plan::TransformSelection::Authored(vec![])
            .apply(
                Some(&input.blobs[image.manifest().layers[1].digest_str()]),
                None,
            )
            .unwrap();
        for empty_mount in [false, true] {
            let parent = tempfile::tempdir().unwrap();
            let destination = parent.path().join("output");
            if empty_mount {
                fs::create_dir(&destination).unwrap();
            }
            let assembled =
                AssembledImage::derived(&cache, &image, &[], &destination, &unchanged).unwrap();
            assert_oci_only(assembled.staged.temporary.path());
            assembled.publish().unwrap();
            assert_oci_only(&destination);
            assert!(destination.join("oci-layout").is_file());
            assert!(destination.join("index.json").is_file());
            assert!(destination.join("blobs/sha256").is_dir());
        }
    }
    #[test]
    fn captured_unchanged_dtb_survives_loss_of_cache_copy() {
        let input = Input::new(false);
        let (cache, image) = input.open();
        let digest = image.manifest().layers[1].digest_str();
        let unchanged = crate::plan::TransformSelection::Authored(vec![])
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
        let unchanged = crate::plan::TransformSelection::Authored(vec![])
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
