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
    assert!(cache.verify_references(&image).is_err());
    store.put(config);
    cache.verify_references(&image).unwrap();
}

impl Store {
    fn scute(
        &self,
        data: &[u8],
        parent: [u8; 32],
        compressed: bool,
    ) -> (crate::artifact::Layer, [u8; 32]) {
        let extent = conglobate::chain::ScuteExtent::new(data.len() as u64, &parent, None).unwrap();
        let root = extent.root(data).unwrap();
        let bytes = if compressed {
            zstd::stream::encode_all(data, 1).unwrap()
        } else {
            data.to_vec()
        };
        let digest = self.put(&bytes);
        let descriptor = crate::artifact::ScuteDescriptor {
            digest: digest.to_string(),
            size: bytes.len() as u64,
            annotations: crate::artifact::ScuteAnnotations {
                data_blocks: None,
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
        let _ = root; // tip derived from scute graph; never stamp tip hash
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
/// Exact index bytes pinned by `tests/distro/context/lock.yaml`.
const CONTEXT_LOCK_INDEX: &[u8] = br#"{"manifests":[{"artifactType":"application/vnd.pichi.artifact.v1+json","digest":"sha256:1111111111111111111111111111111111111111111111111111111111111111","mediaType":"application/vnd.oci.image.manifest.v1+json","platform":{"architecture":"amd64","os":"pichi"},"size":2}],"mediaType":"application/vnd.oci.image.index.v1+json","schemaVersion":2}"#;

#[test]
fn context_lock_index_pin_matches_published_bytes() {
    assert_eq!(CONTEXT_LOCK_INDEX.len(), 343);
    assert_eq!(
        Digest::Sha256(Sha256::digest(CONTEXT_LOCK_INDEX).into()).to_string(),
        "sha256:4e335e141a3f950ed68c68f23c112930c51af5ec9658d1263353eb229a6429d1"
    );
}

#[test]
fn harness_placeholder_loads_supplied_manifest_and_still_checks_layers() {
    let store = Store::new();
    let (layer, root) = store.scute(&[7; 4096], [0; 32], false);
    let bytes = store.manifest_bytes(vec![layer.clone()], root);
    let real = Digest::Sha256(Sha256::digest(&bytes).into());
    let placeholder = Store::placeholder_digest();
    assert_ne!(real, placeholder);
    std::fs::write(store.path(&placeholder), &bytes).unwrap();
    let cache = store.open();
    let image = cache
        .resolve(store.index(vec![store.descriptor(&placeholder, 2)]))
        .unwrap();
    assert_eq!(image.manifest_bytes().bytes(), bytes.as_slice());
    assert_eq!(image.manifest_bytes().digest(), &real);
    cache.verify_content(&image).unwrap().unwrap();

    // Declared size equal to the file is not the placeholder. The digest
    // must then match the bytes.
    let honest_size = store.index(vec![store.descriptor(&placeholder, bytes.len())]);
    assert!(
        cache
            .resolve(honest_size)
            .unwrap_err()
            .to_string()
            .contains("content mismatch")
    );

    let layer_digest: Digest = layer.digest_str().parse().unwrap();
    let layer_path = store.path(&layer_digest);
    let original = std::fs::read(&layer_path).unwrap();
    let mut changed = original.clone();
    changed[0] ^= 1;
    std::fs::write(&layer_path, changed).unwrap();
    assert!(cache.verify_content(&image).is_err());
    std::fs::write(&layer_path, original).unwrap();

    std::fs::write(store.path(&placeholder), b"not-a-manifest").unwrap();
    let err = cache
        .resolve(store.index(vec![store.descriptor(&placeholder, 2)]))
        .unwrap_err()
        .to_string();
    assert!(
        !err.contains("metadata size mismatch"),
        "placeholder path must be read before manifest parsing: {err}"
    );
    assert!(err.contains("invalid OCI JSON"), "{err}");
}

#[test]
fn harness_placeholder_still_rejects_oversized_blobs() {
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
fn published_context_lock_index_resolves_harness_manifest() {
    let store = Store::new();
    let (layer, root) = store.scute(&[7; 4096], [0; 32], false);
    let bytes = store.manifest_bytes(vec![layer], root);
    std::fs::write(store.path(&Store::placeholder_digest()), &bytes).unwrap();
    let pin = store.put(CONTEXT_LOCK_INDEX);
    let image = store
        .open()
        .resolve(pin.to_string().parse().unwrap())
        .unwrap();
    assert_eq!(image.manifest_bytes().bytes(), bytes.as_slice());
    let mut flipped = CONTEXT_LOCK_INDEX.to_vec();
    flipped[0] ^= 1;
    std::fs::write(store.path(&pin), flipped).unwrap();
    assert!(
        store
            .open()
            .resolve(pin.to_string().parse().unwrap())
            .is_err()
    );
}
#[test]
fn chain_verification_orders_mixed_compressed_layers_from_authenticated_root() {
    let store = Store::new();
    let (base, base_root) = store.scute(&[7; 8192], [0; 32], false);
    let (top, top_root) = store.scute(&[9; 4097], base_root, true);
    let image = store.chain_image(vec![top, base], top_root);
    let chain = store.open().verify_chain(&image).unwrap().unwrap();
    assert_eq!(chain.root(), &top_root);
    assert_eq!(chain.scutes().len(), 2);
    assert_eq!(chain.scutes()[0].root(), &base_root);
    assert_eq!(chain.scutes()[1].extent().size(), 4097);
    assert!(chain.scutes()[1].compressed());
}
#[test]
fn duplicate_and_ambiguous_tip_chains_fail() {
    let store = Store::new();
    let (base, root) = store.scute(&[7; 4096], [0; 32], false);
    let (other, _) = store.scute(&[9; 4096], [0; 32], true);
    // Duplicate roots, or two tips both parented at zeros (ambiguous tip).
    for layers in [vec![base.clone(), base.clone()], vec![base, other]] {
        let image = store.chain_image(layers, root);
        assert!(store.open().verify_chain(&image).is_err());
    }
}
#[test]
fn scute_digest_and_decoded_block_count_are_both_enforced() {
    let store = Store::new();
    let (mut layer, root) = store.scute(&[7; 8192], [0; 32], true);
    let crate::artifact::Layer::ScuteZstd(descriptor) = &mut layer else {
        unreachable!()
    };
    descriptor.annotations.data_blocks = Some(1);
    let image = store.chain_image(vec![layer], root);
    assert!(store.open().verify_chain(&image).is_err());
    let (layer, root) = store.scute(&[7; 4096], [0; 32], false);
    let digest: Digest = layer.digest_str().parse().unwrap();
    let image = store.chain_image(vec![layer], root);
    std::fs::write(store.path(&digest), [8; 4096]).unwrap();
    assert!(
        store
            .open()
            .verify_chain(&image)
            .unwrap_err()
            .to_string()
            .contains("content mismatch")
    );
}

#[test]
fn compressed_scutes_reject_invalid_streams_even_with_matching_oci_digests() {
    let store = Store::new();
    let data = [7; 8192];
    let (_, root) = store.scute(&data, [0; 32], true);
    let mut encoder = zstd::stream::Encoder::new(Vec::new(), 1).unwrap();
    encoder.include_checksum(true).unwrap();
    std::io::Write::write_all(&mut encoder, &data).unwrap();
    let valid = encoder.finish().unwrap();
    let mut wrong_checksum = valid.clone();
    *wrong_checksum.last_mut().unwrap() ^= 1;
    let truncated = valid[..valid.len() - 1].to_vec();
    let mut trailing_junk = valid.clone();
    trailing_junk.extend(b"invalid trailing frame");
    for bytes in [wrong_checksum, truncated, trailing_junk] {
        // Digest the invalid compressed bytes themselves so decoder validation,
        // not a stale OCI digest, must reject this fixture.
        let digest = store.put(&bytes);
        let descriptor = crate::artifact::ScuteDescriptor {
            digest: digest.to_string(),
            size: bytes.len() as u64,
            annotations: crate::artifact::ScuteAnnotations {
                data_blocks: Some(2),
                salt: hex::encode([0; 32]),
            },
        };
        let image = store.chain_image(vec![crate::artifact::Layer::ScuteZstd(descriptor)], root);
        assert!(store.open().verify_chain(&image).is_err());
    }
    // Concatenated valid frames describe one complete decoded scute stream.
    let mut frames = zstd::stream::encode_all(&data[..4096], 1).unwrap();
    frames.extend(zstd::stream::encode_all(&data[4096..], 1).unwrap());
    let digest = store.put(&frames);
    let descriptor = crate::artifact::ScuteDescriptor {
        digest: digest.to_string(),
        size: frames.len() as u64,
        annotations: crate::artifact::ScuteAnnotations {
            data_blocks: Some(2),
            salt: hex::encode([0; 32]),
        },
    };
    let image = store.chain_image(vec![crate::artifact::Layer::ScuteZstd(descriptor)], root);
    assert_eq!(
        store.open().verify_chain(&image).unwrap().unwrap().root(),
        &root
    );
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
