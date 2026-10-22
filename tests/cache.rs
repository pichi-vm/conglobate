// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::cache::Cache;
use pichi_artifact::Digest;
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
        let digest = Digest::from_bytes_sha256(bytes);
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
    let receipt = cache.verify(&digest, 7).unwrap();
    assert_eq!(receipt.digest(), &digest);
    assert_eq!(receipt.size(), 7);
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
        let manifest = pichi_artifact::Manifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".into(),
            artifact_type: pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
            config: pichi_artifact::ConfigDescriptor::canonical(),
            layers: Vec::new(),
            annotations: Default::default(),
        };
        let bytes = manifest.to_bytes().unwrap();
        (self.put(&bytes), bytes)
    }
    fn descriptor(&self, digest: &Digest, size: usize) -> serde_json::Value {
        serde_json::json!({"mediaType":"application/vnd.oci.image.manifest.v1+json",
            "artifactType":pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
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
    unrelated["digest"] = Digest::from_bytes_sha256(b"not cached").to_string().into();
    let pin = store.index(vec![unrelated, selected]);
    let image = store.open().resolve(pin).unwrap();
    assert_eq!(image.manifest_bytes().bytes(), bytes);
    assert_eq!(image.index().digest().as_bytes(), pin.bytes());
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
    let config_digest = Digest::from_bytes_sha256(config);
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
    let receipts = cache.verify_references(&image).unwrap();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].digest(), &config_digest);
}
