// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::StagedDirectory;
use crate::artifact::{CARAPACE_ROOT_ANNOTATION, Digest};
use crate::{
    cache::{Cache, CacheError, ResolvedImage},
    image::IndexDigest,
};
use sha2::{Digest as _, Sha256};

/// A self-contained staged image whose copied blobs were authenticated and
/// whose new scutes were sealed against the selected parent root.
/// No mutable staging authority escapes.
/// This is not a successful plan execution or a publication receipt.
#[derive(Debug)]
pub(crate) struct AssembledImage {
    staged: StagedDirectory,
    image: ResolvedImage,
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
        let image = Self::resolve_staging(
            &self.staged,
            &serde_json::to_vec(&manifest)?,
            self.image.architecture(),
        )?;
        anyhow::ensure!(
            self.image.root() == image.root(),
            "build binding changed the scute graph"
        );
        let new_digest = Digest::Sha256(Sha256::digest(image.manifest_bytes().bytes()).into());
        if old_digest != new_digest {
            std::fs::remove_file(self.staged.blob_path(&old_digest))?;
        }
        self.image = image;
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
    pub(crate) fn discard(self) -> std::io::Result<()> {
        self.staged.discard()
    }
}
impl AssembledImage {
    pub(crate) fn components(
        cache: &Cache,
        selection: &crate::pipeline::output::StageOutput<'_, '_>,
        destination: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let staged = StagedDirectory::create(destination)?;
        let assembled = (|| -> anyhow::Result<ResolvedImage> {
            let metadata = match &selection.root {
                Some(root) => root.base,
                None => selection.pmi.as_ref().and_then(|pmi| pmi.inherited()),
            };
            let mut manifest = match metadata {
                Some(image) => {
                    let config = &image.image().manifest().config;
                    if config.is_empty() {
                        staged.store_blob(b"{}")?;
                    } else {
                        staged.copy_blob(cache, &config.digest.parse()?, config.size)?;
                    }
                    serde_json::from_slice::<serde_json::Value>(
                        image.image().manifest_bytes().bytes(),
                    )?
                }
                None => {
                    staged.store_blob(b"{}")?;
                    serde_json::json!({"schemaVersion":2,"mediaType":crate::artifact::MEDIA_TYPE_OCI_MANIFEST_V1,"artifactType":crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,"config":crate::artifact::ConfigDescriptor::canonical(),"layers":[]})
                }
            };
            let annotations = manifest
                .as_object_mut()
                .expect("manifest object")
                .entry("annotations")
                .or_insert_with(|| serde_json::json!({}))
                .as_object_mut()
                .ok_or_else(|| anyhow::anyhow!("invalid manifest annotations"))?;
            if selection.root.is_none() {
                annotations.retain(|name, _| !name.starts_with("dev.pichi.carapace."));
            }
            let mut layers = Vec::new();
            if let Some(root) = &selection.root {
                let mut parent = [0; 32];
                if let Some(base) = root.base {
                    let original: serde_json::Value =
                        serde_json::from_slice(base.image().manifest_bytes().bytes())?;
                    for (index, layer) in base.image().manifest().layers.iter().enumerate() {
                        if matches!(
                            layer,
                            crate::artifact::Layer::Scute(_) | crate::artifact::Layer::ScuteZstd(_)
                        ) {
                            staged.copy_blob(cache, &layer.digest_str().parse()?, layer.size())?;
                            layers.push(original["layers"][index].clone());
                        }
                    }
                    parent = base
                        .root()
                        .ok_or_else(|| anyhow::anyhow!("selected root base has no scutes"))?;
                }
                for scute in &root.produced {
                    anyhow::ensure!(
                        scute.parent() == &parent,
                        "produced scute parent differs from selected root"
                    );
                    staged.store_scute(scute)?;
                    layers.push(serde_json::to_value(crate::artifact::Layer::Scute(
                        scute.descriptor().clone(),
                    ))?);
                    parent = *scute.root();
                }
                anyhow::ensure!(
                    parent == root.hash(),
                    "selected root hash differs from scute ancestry"
                );
                let parameters = Self::verity_annotations();
                if root.base.is_none() {
                    annotations.extend(parameters.as_object().expect("verity annotations").clone());
                } else if !root.produced.is_empty() {
                    for (key, expected) in parameters.as_object().expect("verity annotations") {
                        anyhow::ensure!(
                            annotations.get(key) == Some(expected),
                            "source chain uses incompatible verity parameter {key}"
                        );
                    }
                }
                Self::set_tip_annotation(&mut manifest, Some(parent));
            }
            if let Some(pmi) = &selection.pmi {
                pmi.store(cache, &staged)?;
                layers.push(serde_json::to_value(crate::artifact::Layer::Pmi(
                    pmi.descriptor(),
                ))?);
            }
            if let Some(config) = &selection.bootconfig
                && !config.bytes.is_empty()
            {
                anyhow::ensure!(selection.pmi.is_some(), "bootconfig requires PMI");
                let digest = staged.store_blob(&config.bytes)?;
                layers.push(serde_json::to_value(crate::artifact::Layer::BootConfig(
                    crate::artifact::BootConfigDescriptor {
                        digest: digest.to_string(),
                        size: config.bytes.len() as u64,
                    },
                ))?);
            }
            manifest["layers"] = layers.into();
            Ok(Self::resolve_staging(
                &staged,
                &serde_json::to_vec(&manifest)?,
                crate::cache::HostArchitecture::current()?,
            )?)
        })();
        match assembled {
            Ok(image) => Ok(Self { staged, image }),
            Err(error) => Err(match staged.discard() {
                Ok(()) => error,
                Err(cleanup) => error.context(format!("output staging cleanup: {cleanup}")),
            }),
        }
    }
    fn resolve_staging(
        staged: &StagedDirectory,
        manifest: &[u8],
        architecture: crate::cache::HostArchitecture,
    ) -> Result<ResolvedImage, CacheError> {
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
        output_cache.verify_metadata(&output_image)?;
        // The index is the layout entrypoint, not referenced by its own
        // manifest. Remove the temporary lookup blob after validation.
        std::fs::remove_file(staged.blob_path(&Digest::Sha256(Sha256::digest(&index).into())))?;
        Ok(output_image)
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
            Self::with_firmware(empty_config, crate::firmware::tests::Fixture::native())
        }
        fn with_firmware(empty_config: bool, pmi: Vec<u8>) -> Self {
            let directory = tempfile::tempdir().unwrap();
            let mut blobs = BTreeMap::new();
            let bootconfig = b"corium.root = old\n".to_vec();
            let config = if empty_config {
                b"{}".to_vec()
            } else {
                b"{\"requirements\":{}}".to_vec()
            };
            for bytes in [&pmi, &bootconfig, &config] {
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
                    {"mediaType": "application/vnd.pichi.bootconfig.v1", "digest": Digest::Sha256(Sha256::digest(&bootconfig).into()).to_string(), "size": bootconfig.len()}
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
        fn open(&self) -> (Cache, crate::input::CheckedImage) {
            let cache = Cache::open(self.directory.path()).unwrap();
            let image = cache.resolve(self.pin).unwrap();
            let image = crate::input::CheckedImage::verify(&cache, image).unwrap();
            (cache, image)
        }
        fn selection<'a>(
            &self,
            image: &'a crate::input::CheckedImage,
        ) -> crate::pipeline::output::StageOutput<'a, 'static> {
            crate::pipeline::output::StageOutput {
                root: None,
                pmi: Some(crate::pipeline::pmi::Pmi::Inherited(image)),
                bootconfig: Some(crate::pipeline::boot_config::BootConfig {
                    bytes: self.blobs[image.image().manifest().layers[1].digest_str()].clone(),
                    root: None,
                }),
                config_requested: true,
            }
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
                    fs::write(selected.join("plan.yaml"), "image: registry.test/base:1\n").unwrap();
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
                    let selection = input.selection(&image);
                    let mut assembled =
                        AssembledImage::components(&cache, &selection, &destination).unwrap();
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
                    let stale = AssembledImage::resolve_staging(
                        &assembled.staged,
                        &serde_json::to_vec(&manifest).unwrap(),
                        image.image().architecture(),
                    )
                    .unwrap();
                    let original = Digest::Sha256(
                        Sha256::digest(assembled.image.manifest_bytes().bytes()).into(),
                    );
                    fs::remove_file(assembled.staged.blob_path(&original)).unwrap();
                    assembled.image = stale;
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
    fn opaque_pmi_and_oversized_config_are_rejected_without_output() {
        let input = Input::with_firmware(true, b"opaque firmware".to_vec());
        let cache = Cache::open(input.directory.path()).unwrap();
        let image = cache.resolve(input.pin).unwrap();
        assert!(crate::input::CheckedImage::verify(&cache, image).is_err());
        let input = Input::new(true);
        let (cache, image) = input.open();
        let mut selection = input.selection(&image);
        selection.bootconfig.as_mut().unwrap().bytes =
            format!("key = {}\n", "x".repeat(4096)).into_bytes();
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("output");
        assert!(AssembledImage::components(&cache, &selection, &destination).is_err());
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
    #[test]
    fn overlay_replaces_only_bootconfig_and_is_self_contained() {
        let input = Input::new(false);
        let (cache, image) = input.open();
        let old: serde_json::Value = serde_json::from_slice(&input.manifest).unwrap();
        let old_digest = old["layers"][1]["digest"].as_str().unwrap();
        let mut selection = input.selection(&image);
        let result = b"corium.root = new\n";
        selection.bootconfig.as_mut().unwrap().bytes = result.to_vec();
        let parent = tempfile::tempdir().unwrap();
        let assembled =
            AssembledImage::components(&cache, &selection, &parent.path().join("output")).unwrap();
        let new: serde_json::Value =
            serde_json::from_slice(assembled.image().manifest_bytes().bytes()).unwrap();
        assert_eq!(new["config"], old["config"]);
        assert_eq!(new["layers"][0], old["layers"][0]);
        assert_eq!(new["annotations"], old["annotations"]);
        assert_eq!(new["x-preserved-extension"], old["x-preserved-extension"]);
        assert_ne!(new["layers"][1]["digest"], old["layers"][1]["digest"]);
        let digest = Digest::Sha256(Sha256::digest(result).into());
        assert_eq!(new["layers"][1]["digest"], digest.to_string());
        assert_eq!(
            fs::read(assembled.staged.blob_path(&digest)).unwrap(),
            result
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
        cache.verify_metadata(assembled.image()).unwrap();
        assembled.discard().unwrap();
    }
    #[test]
    fn inheritance_preserves_components_and_metadata_without_source_cache() {
        for empty_config in [true, false] {
            let input = Input::new(empty_config);
            let (cache, image) = input.open();
            let parent = tempfile::tempdir().unwrap();
            let destination = parent.path().join("output");
            let selection = input.selection(&image);
            let assembled = AssembledImage::components(&cache, &selection, &destination).unwrap();
            assert!(!destination.exists());
            assert!(assembled.image().root().is_none());
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(
                    assembled.image().manifest_bytes().bytes()
                )
                .unwrap(),
                serde_json::from_slice::<serde_json::Value>(&input.manifest).unwrap()
            );
            drop(cache);
            fs::remove_dir_all(input.directory.path()).unwrap();
            let blob_directory = assembled.staged.temporary.path().join("blobs/sha256");
            assert_eq!(
                fs::read_dir(&blob_directory).unwrap().count(),
                input.blobs.len()
            );
            for (digest, expected) in input.blobs {
                if expected == input.manifest {
                    continue;
                } // JSON may be serialized canonically.
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
        let selection = input.selection(&image);
        for empty_mount in [false, true] {
            let parent = tempfile::tempdir().unwrap();
            let destination = parent.path().join("output");
            if empty_mount {
                fs::create_dir(&destination).unwrap();
            }
            let assembled = AssembledImage::components(&cache, &selection, &destination).unwrap();
            assert_oci_only(assembled.staged.temporary.path());
            assembled.publish().unwrap();
            assert_oci_only(&destination);
            assert!(destination.join("oci-layout").is_file());
            assert!(destination.join("index.json").is_file());
            assert!(destination.join("blobs/sha256").is_dir());
        }
    }
    #[test]
    fn captured_bootconfig_survives_loss_of_cache_copy() {
        let input = Input::new(false);
        let (cache, image) = input.open();
        let digest = image.image().manifest().layers[1].digest_str();
        let selection = input.selection(&image);
        fs::remove_file(
            input
                .directory
                .path()
                .join("blobs/sha256")
                .join(digest.strip_prefix("sha256:").unwrap()),
        )
        .unwrap();
        let parent = tempfile::tempdir().unwrap();
        let assembled =
            AssembledImage::components(&cache, &selection, &parent.path().join("output")).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(assembled.image().manifest_bytes().bytes())
                .unwrap(),
            serde_json::from_slice::<serde_json::Value>(&input.manifest).unwrap()
        );
        Cache::open(assembled.staged.temporary.path())
            .unwrap()
            .verify_metadata(assembled.image())
            .unwrap();
        assembled.discard().unwrap();
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
    #[test]
    fn missing_reference_retains_only_private_staging_for_cleanup() {
        let input = Input::new(false);
        let (cache, image) = input.open();
        let layer = &image.image().manifest().layers[0];
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
        let selection = input.selection(&image);
        let _error = AssembledImage::components(&cache, &selection, &destination).unwrap_err();
        assert!(!destination.exists());
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 0);
    }
}
