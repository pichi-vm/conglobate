// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::{artifact::Digest, cache::Cache, input::CapturedBuild, tests::ScuteFixture as _};
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
        let digest = Digest::Sha256(Sha256::digest(bytes).into());
        fs::write(
            self.directory
                .path()
                .join("blobs/sha256")
                .join(digest.hex()),
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
            let bytes = crate::firmware::tests::Fixture::native();
            layers.push(serde_json::json!({"mediaType":"application/vnd.pichi.pmi.v1", "digest":self.blob(&bytes), "size":bytes.len()}));
        }
        if let Some(config) = config {
            layers.push(serde_json::json!({"mediaType":"application/vnd.pichi.bootconfig.v1", "digest":self.blob(config.as_bytes()), "size":config.len()}));
        }
        let config = serde_json::to_vec(&serde_json::json!({"source":name})).unwrap();
        let manifest = serde_json::to_vec(&serde_json::json!({"schemaVersion":2, "mediaType":"application/vnd.oci.image.manifest.v1+json", "artifactType":crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1, "config":crate::artifact::ConfigDescriptor::for_config(self.blob(&config),config.len() as u64), "layers":layers,"annotations":annotations})).unwrap();
        let index = serde_json::to_vec(&serde_json::json!({"schemaVersion":2, "mediaType":"application/vnd.oci.image.index.v1+json", "manifests":[{"mediaType":"application/vnd.oci.image.manifest.v1+json", "artifactType":crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1, "digest":self.blob(&manifest),"size":manifest.len(),"platform":{"os":"pichi","architecture":crate::cache::HostArchitecture::current().unwrap().oci_name()}}]})).unwrap();
        self.pins
            .insert(format!("registry.test/{name}:1"), self.blob(&index));
    }
    fn request(&self, recipe: &str) -> (tempfile::TempDir, BuildRequest) {
        let repository = tempfile::tempdir().unwrap();
        fs::write(repository.path().join("plan"), recipe).unwrap();
        let images: BTreeMap<_, _> = self
            .pins
            .iter()
            .map(|(name, pin)| (name, serde_json::json!({"index":pin})))
            .collect();
        fs::write(
            repository.path().join("lock"),
            serde_json::to_vec(&serde_json::json!({"version":1,"images":images})).unwrap(),
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
            "--repository",
            repository.path().to_str().unwrap(),
            "--commit",
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
    fn build(&self, recipe: &str) -> (tempfile::TempDir, BuildRequest, ResolvedBuild) {
        let (repository, request) = self.request(recipe);
        let build = CapturedBuild::try_from(&request)
            .unwrap()
            .resolve(Cache::open(self.directory.path()).unwrap())
            .unwrap();
        (repository, request, build)
    }
    fn replace_pmi(&mut self, reference: &str, bytes: &[u8]) {
        let read = |digest: &str| {
            let digest: Digest = digest.parse().unwrap();
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(
                    self.directory
                        .path()
                        .join("blobs/sha256")
                        .join(digest.hex()),
                )
                .unwrap(),
            )
            .unwrap()
        };
        let mut index = read(&self.pins[reference]);
        let mut manifest = read(index["manifests"][0]["digest"].as_str().unwrap());
        let layer = manifest["layers"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|layer| layer["mediaType"] == "application/vnd.pichi.pmi.v1")
            .unwrap();
        layer["digest"] = self.blob(bytes).into();
        layer["size"] = bytes.len().into();
        let manifest = serde_json::to_vec(&manifest).unwrap();
        index["manifests"][0]["digest"] = self.blob(&manifest).into();
        index["manifests"][0]["size"] = manifest.len().into();
        let index = self.blob(&serde_json::to_vec(&index).unwrap());
        self.pins.insert(reference.into(), index);
    }
    fn selected<'a>(&self, build: &'a ResolvedBuild) -> StageOutput<'a, 'static> {
        assert!(!build.captured().graph().execution_required());
        StageOutput::select(build, &BTreeMap::new(), None).unwrap()
    }
    fn config(output: &StageOutput<'_, '_>) -> crate::artifact::BootConfig {
        std::str::from_utf8(&output.bootconfig.as_ref().unwrap().bytes)
            .unwrap()
            .parse()
            .unwrap()
    }
    fn assert_output(destination: &Path, root: bool, boot: bool, config: bool) {
        let index: serde_json::Value =
            serde_json::from_slice(&fs::read(destination.join("index.json")).unwrap()).unwrap();
        let digest: Digest = index["manifests"][0]["digest"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap();
        let manifest: crate::artifact::Manifest = serde_json::from_slice(
            &fs::read(destination.join("blobs/sha256").join(digest.hex())).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest.carapace_root().unwrap().is_some(), root);
        assert_eq!(
            manifest
                .layers
                .iter()
                .any(|layer| matches!(layer, Layer::Pmi(_))),
            boot
        );
        assert_eq!(
            manifest
                .layers
                .iter()
                .any(|layer| matches!(layer, Layer::BootConfig(_))),
            config
        );
    }
}

#[test]
fn all_five_component_subsets_publish_without_activating_source_roots() {
    let fixture = Fixture::new();
    for (selection, root, boot, config, metadata) in [
        ("{root: registry.test/base:1}", true, false, false, "base"),
        ("{boot: registry.test/base:1}", false, true, false, "base"),
        (
            "{root: registry.test/root:1, boot: registry.test/base:1}",
            true,
            true,
            false,
            "root",
        ),
        (
            "{boot: registry.test/pmi:1, bootconfig: registry.test/boot:1}",
            false,
            true,
            true,
            "pmi",
        ),
        (
            "{root: registry.test/root:1, boot: registry.test/pmi:1, bootconfig: registry.test/boot:1}",
            true,
            true,
            true,
            "root",
        ),
    ] {
        let (_repository, request, build) = fixture.build(&format!("image: {selection}"));
        let selected = fixture.selected(&build);
        let assembled =
            AssembledImage::components(build.cache(), &selected, request.output()).unwrap();
        assert_eq!(
            assembled.image().manifest().annotations["example.org/source"],
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
                    .layers
                    .iter()
                    .filter(|layer| matches!(layer, Layer::Scute(_)))
                    .count(),
                1
            );
        }
        if boot {
            let expected = crate::firmware::tests::Fixture::native();
            let descriptor = selected.pmi.as_ref().unwrap().descriptor();
            assert_eq!(descriptor.digest, fixture.blob(&expected));
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
        ("{root: registry.test/base:1, boot: registry.test/base:1, bootconfig: {from: registry.test/base:1, values: {corium.log: debug}}}".to_owned(), "authored".to_owned()),
        ("{root: registry.test/other:1, boot: registry.test/base:1, bootconfig: registry.test/base:1}".to_owned(), hex::encode(fixture.roots["other"])),
        ("{root: registry.test/root:1, boot: registry.test/pmi:1, bootconfig: {values: {corium.root: ''}}}".to_owned(), hex::encode(fixture.roots["root"])),
    ] {
        let (_repository,_request,build) = fixture.build(&format!("image: {image}"));
        let selected = fixture.selected(&build);
        assert_eq!(Fixture::config(&selected).values()["corium.root"],[expected]);
    }
}

#[test]
fn explicit_deletion_survives_root_join_and_empty_config_is_omitted() {
    let fixture = Fixture::new();
    let (_repository,request,build) = fixture.build("image: {root: registry.test/other:1, boot: registry.test/pmi:1, bootconfig: {from: registry.test/base:1, values: {corium.root: null}}}");
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
    let (_repository,request,build) = fixture.build("image: {boot: registry.test/pmi:1, bootconfig: {from: registry.test/boot:1, values: {corium.root: null}}}");
    let selected = fixture.selected(&build);
    assert!(selected.config_requested);
    assert!(selected.bootconfig.as_ref().unwrap().bytes.is_empty());
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
            "{boot: {from: registry.test/pmi:1, path: /image.pmi}}",
            "PMI file source has no carapace",
        ),
        (
            "{boot: registry.test/pmi:1, bootconfig: registry.test/pmi:1}",
            "bootconfig source has no bootconfig",
        ),
        (
            "{boot: registry.test/pmi:1, bootconfig: {from: registry.test/pmi:1}}",
            "bootconfig source has no bootconfig",
        ),
    ] {
        let (_repository, request) = fixture.request(&format!("image: {image}"));
        let error = CapturedBuild::try_from(&request)
            .unwrap()
            .resolve(Cache::open(fixture.directory.path()).unwrap())
            .unwrap_err();
        assert!(error.to_string().contains(diagnostic), "{error}");
        assert!(!request.output().exists());
    }
}

#[test]
fn empty_authored_config_still_requires_a_reservation() {
    let mut fixture = Fixture::new();
    let machine = if cfg!(target_arch = "aarch64") {
        0xaa64
    } else {
        0x8664
    };
    let mut spec = crate::firmware::tests::Fixture::bootconfig_spec(machine);
    spec["actions"][0]["kind"] = "dt:dtbo".into();
    fixture.replace_pmi(
        "registry.test/pmi:1",
        &crate::firmware::tests::Fixture::sections(
            machine,
            &[(".pmi.vm", &spec)],
            &[(".bconf", 4096)],
        ),
    );
    let (_repository, request, build) = fixture.build("image: {boot: registry.test/pmi:1}");
    fixture
        .selected(&build)
        .assemble(&request, &build)
        .unwrap()
        .image
        .discard()
        .unwrap();
    for config in [
        "{}",
        "{from: registry.test/boot:1, values: {corium.root: null}}",
    ] {
        let (_repository, request, build) = fixture.build(&format!(
            "image: {{boot: registry.test/pmi:1, bootconfig: {config}}}"
        ));
        let error = fixture
            .selected(&build)
            .assemble(&request, &build)
            .unwrap_err();
        assert!(
            error.to_string().contains("no bootconfig reservation"),
            "{error}"
        );
        assert!(!request.output().exists());
    }
}
