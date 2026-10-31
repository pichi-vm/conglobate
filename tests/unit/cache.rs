// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::artifact::Digest;
use conglobate::cache::Cache;
use sha2::{Digest as _, Sha256};
use std::path::PathBuf;

struct Store(tempfile::TempDir);
impl Store {
    fn new() -> Self {
        let store = Self(tempfile::tempdir().unwrap());
        std::fs::create_dir_all(store.0.path().join("blobs/sha256")).unwrap();
        store
    }
    fn path(&self, digest: &Digest) -> PathBuf {
        self.0.path().join("blobs/sha256").join(digest.hex())
    }
    fn put(&self, bytes: &[u8]) -> Digest {
        let digest = Digest::Sha256(Sha256::digest(bytes).into());
        std::fs::write(self.path(&digest), bytes).unwrap();
        digest
    }
    fn open(&self) -> Cache {
        Cache::open(self.0.path()).unwrap()
    }
}

#[test]
fn metadata_is_captured_but_later_reads_are_verified_again() {
    let store = Store::new();
    let digest = store.put(b"content");
    let cache = store.open();
    let metadata = cache.metadata(&digest, Some(7)).unwrap();
    cache.verify(&digest, 7).unwrap();
    std::fs::write(store.path(&digest), b"changed").unwrap();
    assert_eq!(metadata.bytes(), b"content");
    assert!(cache.verify(&digest, 7).is_err());
    assert!(cache.metadata(&digest, None).is_err());
}

#[test]
fn missing_wrong_size_and_non_regular_blobs_fail() {
    let store = Store::new();
    let digest = store.put(b"content");
    let cache = store.open();
    assert!(cache.verify(&digest, 6).is_err());
    assert!(cache.metadata(&digest, Some(8)).is_err());
    std::fs::remove_file(store.path(&digest)).unwrap();
    assert!(cache.metadata(&digest, None).is_err());
    std::fs::create_dir(store.path(&digest)).unwrap();
    assert!(cache.verify(&digest, 0).is_err());
    std::fs::remove_dir(store.path(&digest)).unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        store.path(&digest),
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        0,
    )
    .unwrap();
    assert!(cache.metadata(&digest, None).is_err());
}

#[test]
fn opened_directory_authority_survives_rename_and_rejects_escape() {
    let parent = tempfile::tempdir().unwrap();
    let store = Store::new();
    let digest = store.put(b"content");
    let cache = store.open();
    let moved = parent.path().join("moved");
    std::fs::rename(store.0.path(), &moved).unwrap();
    assert_eq!(cache.metadata(&digest, None).unwrap().bytes(), b"content");
    let blob = moved.join("blobs/sha256").join(digest.hex());
    std::fs::remove_file(&blob).unwrap();
    let external = parent.path().join("external");
    std::fs::write(&external, b"content").unwrap();
    std::os::unix::fs::symlink(&external, &blob).unwrap();
    assert!(cache.metadata(&digest, None).is_err());
}

impl Store {
    fn manifest(&self) -> (Digest, Vec<u8>) {
        let manifest = crate::artifact::Manifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".into(),
            artifact_type: crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
            config: crate::artifact::ConfigDescriptor::canonical(),
            layers: Vec::new(),
            annotations: Default::default(),
        };
        let bytes = serde_json::to_vec(&manifest).unwrap();
        (self.put(&bytes), bytes)
    }
    fn descriptor(&self, digest: &Digest, size: usize) -> serde_json::Value {
        serde_json::json!({"mediaType":"application/vnd.oci.image.manifest.v1+json",
            "artifactType":crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
            "digest":digest.to_string(),"size":size,
            "platform":{"os":"pichi","architecture":conglobate::cache::HostArchitecture::current().unwrap().oci_name()}})
    }
    fn index(&self, entries: Vec<serde_json::Value>) -> conglobate::image::IndexDigest {
        let bytes = serde_json::to_vec(&serde_json::json!({"schemaVersion":2,
            "mediaType":"application/vnd.oci.image.index.v1+json","manifests":entries}))
        .unwrap();
        self.put(&bytes).to_string().parse().unwrap()
    }
}

#[test]
fn pinned_index_selects_only_host_vm_artifact_and_preserves_exact_metadata() {
    let store = Store::new();
    let (digest, bytes) = store.manifest();
    let selected = store.descriptor(&digest, bytes.len());
    let mut unrelated = selected.clone();
    unrelated["platform"]["os"] = "linux".into();
    unrelated["digest"] = Digest::Sha256(Sha256::digest(b"not cached").into())
        .to_string()
        .into();
    let pin = store.index(vec![unrelated, selected]);
    let image = store.open().resolve(pin).unwrap();
    assert_eq!(image.manifest_bytes().bytes(), bytes);
    assert_eq!(image.manifest_bytes().digest(), &digest);
}

#[test]
fn ambiguous_incompatible_and_direct_manifest_pins_fail() {
    let store = Store::new();
    let (digest, bytes) = store.manifest();
    let descriptor = store.descriptor(&digest, bytes.len());
    let cache = store.open();
    assert!(cache.resolve(digest.to_string().parse().unwrap()).is_err());
    let pin = store.index(vec![descriptor.clone(), descriptor.clone()]);
    assert!(
        cache
            .resolve(pin)
            .unwrap_err()
            .to_string()
            .contains("ambiguous")
    );
    let mut incompatible = descriptor.clone();
    incompatible["platform"]["variant"] = "unsupported".into();
    assert!(cache.resolve(store.index(vec![incompatible])).is_err());
    let mut wrong_size = descriptor;
    wrong_size["size"] = 1.into();
    assert!(cache.resolve(store.index(vec![wrong_size])).is_err());
}

#[test]
fn selected_manifest_envelope_is_validated_in_addition_to_shared_artifact_rules() {
    let store = Store::new();
    let (_, bytes) = store.manifest();
    for field in ["schemaVersion", "mediaType"] {
        let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        manifest[field] = if field == "schemaVersion" {
            1.into()
        } else {
            "wrong".into()
        };
        let encoded = serde_json::to_vec(&manifest).unwrap();
        let digest = store.put(&encoded);
        let pin = store.index(vec![store.descriptor(&digest, encoded.len())]);
        assert!(
            store
                .open()
                .resolve(pin)
                .unwrap_err()
                .to_string()
                .contains("envelope")
        );
    }
}

#[test]
fn manifest_resolution_does_not_silently_stand_in_for_content_verification() {
    let store = Store::new();
    let (_, bytes) = store.manifest();
    let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let config = b"{}";
    let config_digest = Digest::Sha256(Sha256::digest(config).into());
    manifest["config"] = serde_json::json!({
        "mediaType":"application/vnd.pichi.config.v1+json",
        "digest":config_digest.to_string(),"size":config.len()});
    let encoded = serde_json::to_vec(&manifest).unwrap();
    let digest = store.put(&encoded);
    let pin = store.index(vec![store.descriptor(&digest, encoded.len())]);
    let cache = store.open();
    let image = cache.resolve(pin).unwrap();
    assert!(cache.verify_metadata(&image).is_err());
    store.put(config);
    cache.verify_metadata(&image).unwrap();
}

impl Store {
    fn scute(
        &self,
        data: &[u8],
        parent: [u8; 32],
        compressed: bool,
    ) -> (crate::artifact::Layer, [u8; 32]) {
        use crate::tests::ScuteFixture as _;
        let (header, tree, root) = data.scute_files(parent);
        let data_blocks = std::num::NonZero::new((data.len() as u64).div_ceil(4096)).unwrap();
        let bytes = if compressed {
            b"opaque compressed input".to_vec()
        } else {
            data.to_vec()
        };
        let digest = self.put(&bytes);
        let directory = self.path(&digest).with_extension("prepared");
        std::fs::create_dir_all(&directory).unwrap();
        let key = hex::encode(Sha256::digest(header.encode()));
        std::fs::write(directory.join(format!("{key}.verity")), tree).unwrap();
        let size = header.parameters.data_size().unwrap();
        if compressed || data.len() as u64 != size {
            let mut padded = data.to_vec();
            padded.resize(size as usize, 0);
            std::fs::write(directory.join(format!("{size}.deflated")), padded).unwrap();
        }
        let descriptor = crate::artifact::ScuteDescriptor {
            digest: digest.to_string(),
            size: bytes.len() as u64,
            annotations: crate::artifact::ScuteAnnotations {
                data_blocks,
                salt: hex::encode(parent),
            },
        };
        (
            if compressed {
                crate::artifact::Layer::ScuteZstd(descriptor)
            } else {
                crate::artifact::Layer::Scute(descriptor)
            },
            root,
        )
    }
    fn manifest_bytes(&self, layers: Vec<crate::artifact::Layer>, root: [u8; 32]) -> Vec<u8> {
        let manifest = crate::artifact::Manifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".into(),
            artifact_type: crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
            config: crate::artifact::ConfigDescriptor::canonical(),
            layers,
            annotations: [
                ("algo", "sha256"),
                ("data-block-size", "4096"),
                ("hash-block-size", "4096"),
                ("version", "1"),
                ("hash-type", "1"),
                ("hash", &hex::encode(root)),
            ]
            .into_iter()
            .map(|(key, value)| (format!("dev.pichi.carapace.verity.{key}"), value.to_owned()))
            .collect(),
        };
        serde_json::to_vec(&manifest).unwrap()
    }
    fn chain_image(
        &self,
        layers: Vec<crate::artifact::Layer>,
        root: [u8; 32],
    ) -> conglobate::cache::ResolvedImage {
        let bytes = self.manifest_bytes(layers, root);
        let digest = self.put(&bytes);
        self.open()
            .resolve(self.index(vec![self.descriptor(&digest, bytes.len())]))
            .unwrap()
    }
    fn placeholder_digest() -> Digest {
        "sha256:1111111111111111111111111111111111111111111111111111111111111111"
            .parse()
            .unwrap()
    }
}
const METADATA_LOCK: &str = include_str!("fixtures/metadata-lock.yaml");

#[test]
fn metadata_lock_index_authenticates_its_manifest_descriptor() {
    let index = METADATA_LOCK
        .lines()
        .find_map(|line| line.strip_prefix("# index-bytes: "))
        .unwrap();
    let manifest = METADATA_LOCK
        .lines()
        .find_map(|line| line.strip_prefix("# manifest-bytes: "))
        .unwrap();
    let index: serde_json::Value = serde_json::from_str(index).unwrap();
    assert_eq!(index["manifests"][0]["size"], manifest.len());
    assert_eq!(
        index["manifests"][0]["digest"],
        Digest::Sha256(Sha256::digest(manifest.as_bytes()).into()).to_string()
    );
}

#[test]
fn former_harness_descriptor_rejects_wrong_size_and_digest() {
    let store = Store::new();
    let (_, bytes) = store.manifest();
    let placeholder = Store::placeholder_digest();
    let cache = store.open();
    std::fs::write(store.path(&placeholder), &bytes).unwrap();
    for (size, error) in [
        (2, "metadata size mismatch"),
        (bytes.len(), "content mismatch"),
    ] {
        assert!(
            cache
                .resolve(store.index(vec![store.descriptor(&placeholder, size)]))
                .unwrap_err()
                .to_string()
                .contains(error)
        );
    }
    // Even the formerly special digest AND length must authenticate their bytes.
    std::fs::write(store.path(&placeholder), b"{}").unwrap();
    assert!(
        cache
            .resolve(store.index(vec![store.descriptor(&placeholder, 2)]))
            .unwrap_err()
            .to_string()
            .contains("content mismatch")
    );
}

#[test]
fn exporting_an_authenticated_manifest_still_checks_layer_copies() {
    let store = Store::new();
    let (layer, root) = store.scute(&[7; 4096], [0; 32], false);
    let image = store.chain_image(vec![layer.clone()], root);
    let cache = store.open();
    cache.verify_metadata(&image).unwrap();
    let digest = layer.digest_str().parse().unwrap();
    let mut changed = std::fs::read(store.path(&digest)).unwrap();
    changed[0] ^= 1;
    std::fs::write(store.path(&digest), changed).unwrap();
    cache.verify_metadata(&image).unwrap();
    assert!(
        cache
            .copy_verified(&digest, layer.size(), &mut std::io::sink())
            .is_err()
    );
}

#[test]
fn former_harness_descriptor_rejects_oversized_blobs() {
    let store = Store::new();
    let placeholder = Store::placeholder_digest();
    std::fs::write(store.path(&placeholder), vec![0; 16 * 1024 * 1024 + 1]).unwrap();
    let err = store
        .open()
        .resolve(store.index(vec![store.descriptor(&placeholder, 2)]))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("metadata size mismatch or limit exceeded"),
        "{err}"
    );
}

#[cfg(target_arch = "x86_64")]
#[test]
fn fixture_lock_authenticates_manifest_and_index_bytes() {
    use conglobate::{document::YamlDocument, lock::ImageLock};
    let store = Store::new();
    let index = METADATA_LOCK
        .lines()
        .find_map(|line| line.strip_prefix("# index-bytes: "))
        .unwrap();
    let manifest = METADATA_LOCK
        .lines()
        .find_map(|line| line.strip_prefix("# manifest-bytes: "))
        .unwrap();
    let manifest_digest = store.put(manifest.as_bytes());
    let index_digest = store.put(index.as_bytes());
    let lock = ImageLock::try_from(YamlDocument::from(METADATA_LOCK)).unwrap();
    let pin = lock
        .resolve(&"qualification.local/metadata:1".parse().unwrap())
        .unwrap();
    let cache = store.open();
    assert_eq!(
        cache.resolve(pin).unwrap().manifest_bytes().bytes(),
        manifest.as_bytes()
    );
    for (digest, bytes) in [
        (manifest_digest, manifest.as_bytes()),
        (index_digest, index.as_bytes()),
    ] {
        let mut flipped = bytes.to_vec();
        flipped[0] ^= 1;
        std::fs::write(store.path(&digest), flipped).unwrap();
        assert!(cache.resolve(pin).is_err());
        std::fs::write(store.path(&digest), bytes).unwrap();
    }
}
#[test]
fn manifest_tip_is_captured_without_scanning_encoded_scutes() {
    let store = Store::new();
    let (base, base_root) = store.scute(&[7; 8192], [0; 32], false);
    let (top, top_root) = store.scute(&[9; 4097], base_root, true);
    let layers = vec![base, top];
    let image = store.chain_image(layers.clone(), top_root);
    assert_eq!(image.root(), Some(top_root));
    for layer in layers {
        std::fs::remove_file(store.path(&layer.digest_str().parse().unwrap())).unwrap();
    }
    // Neither encoded blobs nor regenerated hash trees are needed for metadata.
    // Actual filesystem access must open prepared files and verify reads instead.
    store.open().verify_metadata(&image).unwrap();
}

#[test]
fn reconstruction_annotations_do_not_configure_execution() {
    let store = Store::new();
    let (layer, root) = store.scute(&[7; 8192], [0; 32], true);
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&store.manifest_bytes(vec![layer], root)).unwrap();
    manifest["annotations"]["dev.pichi.carapace.verity.version"] = "99".into();
    manifest["annotations"]["dev.pichi.carapace.verity.hash-type"] = "0".into();
    manifest["annotations"]["dev.pichi.carapace.verity.algo"] = "untrusted hint".into();
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let digest = store.put(&bytes);
    let cache = store.open();
    let image = cache
        .resolve(store.index(vec![store.descriptor(&digest, bytes.len())]))
        .unwrap();
    assert_eq!(image.root(), Some(root));
    cache.verify_metadata(&image).unwrap();
}

#[test]
fn verified_copy_propagates_destination_failure() {
    struct Reject;
    impl std::io::Write for Reject {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::StorageFull,
                "fixture full",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let store = Store::new();
    let digest = store.put(b"content");
    let error = store
        .open()
        .copy_verified(&digest, 7, &mut Reject)
        .unwrap_err();
    assert!(
        matches!(error, conglobate::cache::CacheError::Io(error) if error.kind() == std::io::ErrorKind::StorageFull)
    );
}

#[test]
fn invalid_manifest_json_retains_decoder_error_source() {
    let store = Store::new();
    let bytes = b"not JSON";
    let digest = store.put(bytes);
    let index = store.index(vec![store.descriptor(&digest, bytes.len())]);
    let error = store.open().resolve(index).unwrap_err();
    let source = std::error::Error::source(&error).unwrap();
    assert!(source.downcast_ref::<serde_json::Error>().is_some());
    assert_eq!(error.to_string(), format!("invalid OCI JSON: {source}"));
}

#[test]
fn decoded_manifest_still_requires_structural_validation() {
    let store = Store::new();
    let (_, bytes) = store.manifest();
    let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    manifest["artifactType"] = "application/unknown".into();
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let digest = store.put(&bytes);
    let index = store.index(vec![store.descriptor(&digest, bytes.len())]);
    let error = store.open().resolve(index).unwrap_err();
    let source = std::error::Error::source(&error).unwrap();
    assert!(
        source
            .downcast_ref::<crate::artifact::ManifestValidationError>()
            .is_some()
    );
}

#[test]
fn prepared_discovery_requires_manifest_order_and_complete_chain() {
    use conglobate::cache::FromIndex as _;
    let store = Store::new();
    let (base, base_root) = store.scute(&[1; 8192], [0; 32], false);
    let (middle, middle_root) = store.scute(&[2; 4097], base_root, true);
    let (tip, tip_root) = store.scute(&[3; 8192], middle_root, false);
    let cache = store.open();
    for (layers, valid) in [
        (vec![base.clone(), middle.clone(), tip.clone()], true),
        (vec![base.clone(), tip.clone(), middle], false),
        (vec![base.clone(), tip], false),
        (vec![base], false),
    ] {
        let image = store.chain_image(layers, tip_root);
        let result = carapace::Chain::from_index(&cache, &image);
        assert_eq!(result.is_ok(), valid, "{result:?}");
        if let Ok(chain) = result {
            assert_eq!(chain.root.as_ref(), &tip_root);
            assert_eq!(chain.scutes.len(), 3);
        }
    }
}

#[test]
fn prepared_variants_are_selected_by_root_and_payloads_remain_lazy() {
    use conglobate::cache::FromIndex as _;
    use std::os::unix::fs::FileExt as _;
    let store = Store::new();
    // The same blob has valid prepared variants with different parents.
    let (base, base_root) = store.scute(&[4; 8192], [0; 32], false);
    let (tip, tip_root) = store.scute(&[4; 8192], base_root, false);
    // Other images can retain variants outside Carapace's hash convention.
    // Their presence must not hide the matching normal-format preparation.
    use crate::tests::ScuteFixture as _;
    use devmap_verity::Format as _;
    let (mut other_header, _, _) = [4; 8192].scute_files([0; 32]);
    other_header.parameters.hash_type = devmap_verity::HashType::ChromeOs;
    let mut other_tree = std::io::Cursor::new(Vec::new());
    other_header
        .format([4; 8192].as_slice(), &mut other_tree)
        .unwrap();
    let directory = store
        .path(&tip.digest_str().parse().unwrap())
        .with_extension("prepared");
    std::fs::write(
        directory.join("other-format.verity"),
        other_tree.into_inner(),
    )
    .unwrap();
    let cache = store.open();
    for (layers, root, count) in [
        (vec![base.clone()], base_root, 1),
        (vec![base, tip.clone()], tip_root, 2),
    ] {
        let image = store.chain_image(layers, root);
        let chain = carapace::Chain::from_index(&cache, &image).unwrap();
        assert_eq!(chain.scutes.len(), count);
        assert!(chain.scutes[0].data.write_all_at(b"x", 0).is_err());
    }
    let path = store.path(&tip.digest_str().parse().unwrap());
    let (base, _) = store.scute(&[4; 8192], [0; 32], false);
    let image = store.chain_image(vec![base], base_root);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .write_all_at(b"x", 4096)
        .unwrap();
    let chain = carapace::Chain::from_index(&cache, &image).unwrap();
    // Discovery retained the opened inode when the cache pathname is replaced.
    std::fs::remove_file(&path).unwrap();
    std::fs::write(path, [9; 8192]).unwrap();
    let mut byte = [0];
    chain.scutes[0].data.read_exact_at(&mut byte, 4096).unwrap();
    assert_eq!(byte, [b'x']);
}

#[test]
fn prepared_root_and_header_mutations_are_rejected() {
    use conglobate::cache::FromIndex as _;
    let store = Store::new();
    let (base, root) = store.scute(&[1; 8192], [0; 32], false);
    let image = store.chain_image(vec![base.clone()], root);
    let directory = store
        .path(&base.digest_str().parse().unwrap())
        .with_extension("prepared");
    let path = std::fs::read_dir(&directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let bytes = std::fs::read(&path).unwrap();
    let cache = store.open();
    for offset in [8, 88, 4096] {
        let mut changed = bytes.clone();
        changed[offset] ^= 1;
        std::fs::write(&path, changed).unwrap();
        assert!(carapace::Chain::from_index(&cache, &image).is_err());
    }
    std::fs::write(&path, &bytes).unwrap();
    std::fs::write(directory.join("duplicate.verity"), &bytes).unwrap();
    assert!(carapace::Chain::from_index(&cache, &image).is_err());
}

#[test]
fn compressed_and_partial_inputs_use_prepared_data_including_single_block_trees() {
    use conglobate::cache::FromIndex as _;
    use std::os::unix::fs::FileExt as _;
    for (size, compressed) in [
        (4096, false),
        (4097, false),
        (4097, true),
        (129 * 4096, false),
    ] {
        let store = Store::new();
        let (base, root) = store.scute(&vec![3; size], [0; 32], compressed);
        let image = store.chain_image(vec![base.clone()], root);
        if compressed {
            std::fs::remove_file(store.path(&base.digest_str().parse().unwrap())).unwrap();
        }
        let chain = carapace::Chain::from_index(&store.open(), &image).unwrap();
        let mut bytes = vec![0; size.div_ceil(4096) * 4096];
        chain.scutes[0].data.read_exact_at(&mut bytes, 0).unwrap();
        assert!(bytes[..size].iter().all(|b| *b == 3));
        assert!(bytes[size..].iter().all(|b| *b == 0));
    }
}
