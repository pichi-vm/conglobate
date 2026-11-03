// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CompletedImage, boot_config::BootConfig, pmi::Pmi, root::Root};
use crate::{
    contract::plan::{Boot, Image, ImageRef, StageRef},
    image::ImageReference,
    input::{CheckedImage, ResolvedBuild},
    output::AssembledImage,
    request::BuildRequest,
    storage::{Runtime, Workspace},
};
use anyhow::{anyhow, ensure};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone)]
pub(crate) struct StageOutput<'b, 'w> {
    pub(crate) root: Option<Root<'b, 'w>>,
    pub(crate) pmi: Option<Pmi<'b, 'w>>,
    pub(crate) bootconfig: Option<BootConfig>,
}
impl<'b, 'w> StageOutput<'b, 'w> {
    pub(super) fn remote(base: &'b CheckedImage, build: &ResolvedBuild) -> anyhow::Result<Self> {
        let has_boot = base.image().manifest().layers().iter().any(|layer| {
            matches!(
                layer.media_type().as_ref(),
                crate::artifact::MEDIA_TYPE_PICHI_PMI_V1
            )
        });
        let config = base
            .bootconfig(build.cache())?
            .map(|bytes| BootConfig::inherited(bytes.bytes().to_vec(), base.root()))
            .transpose()?
            .filter(|config| !config.bytes.is_empty());
        Ok(Self {
            root: base.root().map(|_| Root {
                base: Some(base),
                produced: vec![],
            }),
            pmi: has_boot.then_some(Pmi::Inherited(base)),
            bootconfig: config,
        })
    }
    pub(super) fn resolve(
        reference: &ImageRef,
        build: &'b ResolvedBuild,
        outputs: &BTreeMap<StageRef, Self>,
    ) -> anyhow::Result<Self> {
        match reference {
            ImageRef::Local(name) => outputs
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow!("stage {name} has not completed")),
            ImageRef::Remote(remote) => Self::remote(
                build
                    .images()
                    .get(&ImageReference::from(remote))
                    .ok_or_else(|| anyhow!("remote source is not resolved"))?,
                build,
            ),
        }
    }
    pub(super) fn bind(&mut self) -> anyhow::Result<()> {
        if let Some(root) = &self.root
            && let Some(config) = &mut self.bootconfig
        {
            config.bind(root.hash())?;
        }
        Ok(())
    }
    pub(super) fn select(
        build: &'b ResolvedBuild,
        outputs: &BTreeMap<StageRef, Self>,
        reader: Option<(&'w Workspace, &Runtime<'_>)>,
    ) -> anyhow::Result<Self> {
        let plan = build.captured().graph().plan();
        let resolve = |reference| Self::resolve(reference, build, outputs);
        let mut output = match &plan.image {
            Image::Shorthand(reference) => resolve(reference)?,
            Image::Components { root, boot } => {
                let root = root
                    .as_ref()
                    .map(|reference| {
                        resolve(reference)?
                            .root
                            .ok_or_else(|| anyhow!("selected source has no root"))
                    })
                    .transpose()?;
                let (pmi, bootconfig) = match boot {
                    None => (None, None),
                    Some(Boot::Extract { extract, define }) => {
                        let source = resolve(&extract.from)?
                            .root
                            .ok_or_else(|| anyhow!("PMI file source has no root"))?;
                        let (workspace, runtime) = reader
                            .ok_or_else(|| anyhow!("PMI extraction requires a reader runtime"))?;
                        let pmi = Pmi::File(Arc::new(source.read(
                            build.cache(),
                            workspace,
                            runtime,
                            |volume, budget| {
                                Ok(volume.extract_artifact(
                                    extract.path.as_ref(),
                                    workspace,
                                    budget,
                                )?)
                            },
                        )?));
                        let config = BootConfig::overlay(
                            None,
                            define.iter().map(|(key, value)| (key, Some(value))),
                        )?;
                        (Some(pmi), Some(config))
                    }
                    Some(boot) => {
                        let reference = match boot {
                            Boot::Shorthand(source) => source,
                            Boot::Inherit { inherit }
                            | Boot::InheritDefined { inherit, .. }
                            | Boot::InheritExtended { inherit, .. } => inherit,
                            Boot::Extract { .. } => unreachable!(),
                        };
                        let source = resolve(reference)?;
                        let pmi = source
                            .pmi
                            .ok_or_else(|| anyhow!("selected source has no PMI"))?;
                        let config = match boot {
                            Boot::InheritDefined { define, .. } => Some(BootConfig::overlay(
                                None,
                                define.iter().map(|(key, value)| (key, Some(value))),
                            )?),
                            Boot::InheritExtended { extend, .. } => Some(BootConfig::overlay(
                                source.bootconfig,
                                extend.iter().map(|(key, value)| (key, value.as_ref())),
                            )?),
                            _ => source.bootconfig,
                        };
                        (Some(pmi), config)
                    }
                };
                Self {
                    root,
                    pmi,
                    bootconfig: bootconfig.filter(|config| !config.bytes.is_empty()),
                }
            }
        };
        ensure!(
            output.root.is_some() || output.pmi.is_some(),
            "image contains neither root nor PMI"
        );
        ensure!(
            output.bootconfig.is_none() || output.pmi.is_some(),
            "bootconfig requires PMI"
        );
        if let Some(root) = &output.root {
            let hash = Some(root.hash());
            let changed = !root.produced.is_empty()
                || output.pmi.as_ref().is_some_and(|pmi| pmi.root() != hash)
                || output
                    .bootconfig
                    .as_ref()
                    .is_some_and(|config| config.root != hash);
            if changed {
                output.bind()?;
            }
        }
        Ok(output)
    }
    pub(super) fn assemble(
        self,
        request: &BuildRequest,
        build: &ResolvedBuild,
    ) -> anyhow::Result<CompletedImage> {
        let image = AssembledImage::components(build.cache(), &self, request.output())?
            .bind_build(request, build.captured())?;
        Ok(CompletedImage { image })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ManifestExt as _;
    use crate::{cache::Cache, cache::tests::ScuteFixture as _, input::CapturedBuild};
    use oci_spec::image::Digest;
    use sha2::{Digest as _, Sha256};
    use std::{fs, path::Path, process::Command};

    struct Fixture {
        directory: tempfile::TempDir,
        pins: BTreeMap<String, String>,
        roots: BTreeMap<String, [u8; 32]>,
    }
    impl Fixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            fs::create_dir_all(directory.path().join("blobs/sha256")).unwrap();
            let mut fixture = Self {
                directory,
                pins: BTreeMap::new(),
                roots: BTreeMap::new(),
            };
            fixture.image(
                "base",
                Some(1),
                true,
                Some("corium.root = authored\ncorium.child = keep\n"),
            );
            fixture.image("other", Some(2), true, Some("corium.root = old-other\n"));
            fixture.image("root", Some(3), false, None);
            fixture.image("pmi", None, true, None);
            fixture.image("boot", None, true, Some("corium.root = \"\"\n"));
            fixture
        }
        fn blob(&self, bytes: &[u8]) -> String {
            let digest = format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
                .parse::<Digest>()
                .expect("SHA-256 digest");
            fs::write(
                self.directory
                    .path()
                    .join("blobs/sha256")
                    .join(digest.digest()),
                bytes,
            )
            .unwrap();
            digest.to_string()
        }
        fn image(&mut self, name: &str, seed: Option<u8>, boot: bool, config: Option<&str>) {
            let mut layers = Vec::new();
            let mut annotations = serde_json::json!({"example.org/source":name});
            if let Some(seed) = seed {
                let bytes = vec![seed; 8192];
                let hash = bytes.scute_root([0; 32]);
                self.roots.insert(name.into(), hash);
                layers.push(serde_json::json!({"mediaType":"application/vnd.pichi.scute.v1", "digest":self.blob(&bytes), "size":bytes.len(), "annotations":{"dev.pichi.scute.verity.data-blocks":"2", "dev.pichi.scute.verity.salt":"00".repeat(32)}}));
                for (key, value) in [
                    ("algo", "sha256"),
                    ("data-block-size", "4096"),
                    ("hash-block-size", "4096"),
                    ("version", "1"),
                    ("hash-type", "1"),
                ] {
                    annotations[format!("dev.pichi.carapace.verity.{key}")] = value.into();
                }
                annotations["dev.pichi.carapace.verity.hash"] = hex::encode(hash).into();
            }
            if boot {
                let bytes = b"opaque PMI".to_vec();
                layers.push(serde_json::json!({"mediaType":"application/vnd.pichi.pmi.v1", "digest":self.blob(&bytes), "size":bytes.len()}));
            }
            if let Some(config) = config {
                let config = ruzstd::encoding::compress_to_vec(
                    config.as_bytes(),
                    ruzstd::encoding::CompressionLevel::Fastest,
                );
                layers.push(serde_json::json!({"mediaType":"application/vnd.pichi.bootconfig.v1+zstd", "digest":self.blob(&config), "size":config.len()}));
            }
            let config = serde_json::to_vec(&serde_json::json!({"source":name})).unwrap();
            let manifest = serde_json::to_vec(&serde_json::json!({"schemaVersion":2, "mediaType":"application/vnd.oci.image.manifest.v1+json", "artifactType":crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1, "config":oci_spec::image::Descriptor::new(crate::contract::resources::MEDIA_TYPE_PICHI_CONFIG_V1.into(), config.len() as u64, (self.blob(&config)).parse::<oci_spec::image::Digest>().expect("config digest")), "layers":layers,"annotations":annotations})).unwrap();
            let index = serde_json::to_vec(&serde_json::json!({"schemaVersion":2, "mediaType":"application/vnd.oci.image.index.v1+json", "manifests":[{"mediaType":"application/vnd.oci.image.manifest.v1+json", "artifactType":crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1, "digest":self.blob(&manifest),"size":manifest.len(),"platform":{"os":"pichi","architecture":crate::cache::HostArchitecture::current().unwrap().oci_name()}}]})).unwrap();
            self.pins
                .insert(format!("registry.test/{name}:1"), self.blob(&index));
        }
        fn request(&self, plan: &str) -> (tempfile::TempDir, BuildRequest) {
            let repository = tempfile::tempdir().unwrap();
            fs::write(repository.path().join("plan"), plan).unwrap();
            fs::write(
                repository.path().join("lock"),
                serde_json::to_vec(&serde_json::json!({"version":1,"images":self.pins})).unwrap(),
            )
            .unwrap();
            for args in [
                vec!["init", "-q"],
                vec!["add", "."],
                vec![
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@invalid",
                    "commit",
                    "-qm",
                    "Capture composition",
                ],
            ] {
                let result = Command::new("git")
                    .arg("-C")
                    .arg(repository.path())
                    .args(args)
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
            }
            let commit = Command::new("git")
                .arg("-C")
                .arg(repository.path())
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap();
            let commit = String::from_utf8(commit.stdout).unwrap();
            let request = BuildRequest::parse_from([
                "conglobate",
                "build",
                repository.path().to_str().unwrap(),
                "--ref",
                commit.trim(),
                "--plan",
                "plan",
                "--lock",
                "lock",
                "--cache",
                self.directory.path().to_str().unwrap(),
                "--output",
                repository.path().join("output").to_str().unwrap(),
            ])
            .unwrap();
            (repository, request)
        }
        fn build(&self, plan: &str) -> (tempfile::TempDir, BuildRequest, ResolvedBuild) {
            let (repository, request) = self.request(plan);
            let build = CapturedBuild::try_from(&request)
                .unwrap()
                .resolve(Cache::open(self.directory.path()).unwrap())
                .unwrap();
            (repository, request, build)
        }
        fn selected<'a>(&self, build: &'a ResolvedBuild) -> StageOutput<'a, 'static> {
            assert!(!build.captured().graph().execution_required());
            StageOutput::select(build, &BTreeMap::new(), None).unwrap()
        }
        fn config(output: &StageOutput<'_, '_>) -> crate::boot_config::BootConfig {
            BootConfig::decode(&output.bootconfig.as_ref().unwrap().bytes).unwrap()
        }
        fn assert_output(destination: &Path, root: bool, boot: bool, config: bool) {
            let index: serde_json::Value =
                serde_json::from_slice(&fs::read(destination.join("index.json")).unwrap()).unwrap();
            let digest: Digest = index["manifests"][0]["digest"]
                .as_str()
                .unwrap()
                .parse()
                .unwrap();
            let manifest: oci_spec::image::ImageManifest = serde_json::from_slice(
                &fs::read(destination.join("blobs/sha256").join(digest.digest())).unwrap(),
            )
            .unwrap();
            assert_eq!(manifest.carapace_root().unwrap().is_some(), root);
            assert_eq!(
                manifest.layers().iter().any(|layer| matches!(
                    layer.media_type().as_ref(),
                    crate::artifact::MEDIA_TYPE_PICHI_PMI_V1
                )),
                boot
            );
            assert_eq!(
                manifest.layers().iter().any(|layer| matches!(
                    layer.media_type().as_ref(),
                    crate::artifact::MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD
                )),
                config
            );
        }
    }

    #[test]
    fn all_five_component_subsets_publish_without_activating_source_roots() {
        let fixture = Fixture::new();
        for (selection, root, boot, config, metadata) in [
            ("{root: registry.test/base:1}", true, false, false, "base"),
            ("{boot: registry.test/pmi:1}", false, true, false, "pmi"),
            (
                "{root: registry.test/root:1, boot: registry.test/pmi:1}",
                true,
                true,
                false,
                "root",
            ),
            ("{boot: registry.test/boot:1}", false, true, true, "boot"),
            (
                "{root: registry.test/root:1, boot: registry.test/boot:1}",
                true,
                true,
                true,
                "root",
            ),
        ] {
            let (_repository, request, build) =
                fixture.build(&format!("version: 1\nimage: {selection}"));
            let selected = fixture.selected(&build);
            let assembled =
                AssembledImage::components(build.cache(), &selected, request.output()).unwrap();
            assert_eq!(
                assembled.image().manifest().annotations().as_ref().unwrap()["example.org/source"],
                metadata
            );
            // Inputs contain no prepared scutes. Reading a source root would fail.
            if root {
                let expected = &fixture.roots[metadata];
                assert_eq!(assembled.image().root(), Some(*expected));
                assert_eq!(
                    assembled
                        .image()
                        .manifest()
                        .layers()
                        .iter()
                        .filter(|layer| matches!(
                            layer.media_type().as_ref(),
                            crate::artifact::MEDIA_TYPE_PICHI_SCUTE_V1
                        ))
                        .count(),
                    1
                );
            }
            if boot {
                let expected = b"opaque PMI".to_vec();
                let descriptor = selected.pmi.as_ref().unwrap().descriptor();
                assert_eq!(descriptor.digest().to_string(), fixture.blob(&expected));
            }
            assembled.publish().unwrap();
            Fixture::assert_output(request.output(), root, boot, config);
        }
    }

    #[test]
    fn unchanged_root_config_overlay_does_not_rebind_but_joined_roots_do() {
        let fixture = Fixture::new();
        for (image, expected) in [
        ("registry.test/base:1".to_owned(), "authored".to_owned()),
        ("{root: registry.test/base:1, boot: {inherit: registry.test/base:1, extend: {corium.log: debug}}}".to_owned(), "authored".to_owned()),
        ("{root: registry.test/other:1, boot: registry.test/base:1}".to_owned(), hex::encode(fixture.roots["other"])),
        ("{root: registry.test/root:1, boot: {inherit: registry.test/pmi:1, define: {corium.root: ''}}}".to_owned(), hex::encode(fixture.roots["root"])),
    ] {
        let (_repository,_request,build) = fixture.build(&format!("version: 1\nimage: {image}"));
        let selected = fixture.selected(&build);
        assert_eq!(Fixture::config(&selected).values()["corium.root"],[expected]);
    }
    }

    #[test]
    fn explicit_deletion_survives_root_join_and_empty_config_is_omitted() {
        let fixture = Fixture::new();
        let (_repository,request,build) = fixture.build("version: 1\nimage: {root: registry.test/other:1, boot: {inherit: registry.test/base:1, extend: {corium.root: null}}}");
        let selected = fixture.selected(&build);
        let config = Fixture::config(&selected);
        assert!(!config.values().contains_key("corium.root"));
        assert_eq!(config.values()["corium.child"], ["keep"]);
        selected
            .assemble(&request, &build)
            .unwrap()
            .image
            .discard()
            .unwrap();
        let (_repository, request, build) = fixture.build(
        "version: 1\nimage: {boot: {inherit: registry.test/boot:1, extend: {corium.root: null}}}",
    );
        let selected = fixture.selected(&build);
        assert!(selected.bootconfig.is_none());
        selected
            .assemble(&request, &build)
            .unwrap()
            .image
            .publish()
            .unwrap();
        Fixture::assert_output(request.output(), false, true, false);
    }

    #[test]
    fn unavailable_selected_components_fail_before_runtime_or_publication() {
        let fixture = Fixture::new();
        for (image, diagnostic) in [
            ("{root: registry.test/pmi:1}", "root source has no carapace"),
            ("{boot: registry.test/root:1}", "boot source has no PMI"),
            (
                "{boot: {extract: {from: registry.test/pmi:1, path: /image.pmi}}}",
                "PMI file source has no carapace",
            ),
        ] {
            let (_repository, request) = fixture.request(&format!("version: 1\nimage: {image}"));
            let error = CapturedBuild::try_from(&request)
                .unwrap()
                .resolve(Cache::open(fixture.directory.path()).unwrap())
                .unwrap_err();
            assert!(error.to_string().contains(diagnostic), "{error}");
            assert!(!request.output().exists());
        }
    }

    #[test]
    fn define_replaces_inherited_configuration_and_empty_definition_clears_it() {
        let fixture = Fixture::new();
        for (boot, expected) in [
            ("registry.test/base:1", Some("corium.root")),
            ("{inherit: registry.test/base:1}", Some("corium.root")),
            (
                "{inherit: registry.test/base:1, define: {kernel.quiet: []}}",
                Some("kernel.quiet"),
            ),
            ("{inherit: registry.test/base:1, define: {}}", None),
        ] {
            let (_repository, request, build) =
                fixture.build(&format!("version: 1\nimage: {{boot: {boot}}}"));
            let selected = fixture.selected(&build);
            if let Some(key) = expected {
                let config = Fixture::config(&selected);
                assert!(config.values().contains_key(key));
                if key == "kernel.quiet" {
                    assert!(!config.values().contains_key("corium.root"));
                }
            } else {
                assert!(selected.bootconfig.is_none());
            }
            selected
                .assemble(&request, &build)
                .unwrap()
                .image
                .publish()
                .unwrap();
            Fixture::assert_output(request.output(), false, true, expected.is_some());
        }
    }
}
