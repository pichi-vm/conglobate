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
