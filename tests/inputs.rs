// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{cache::Cache, context::Context, recipe::IndexDigest};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> String {
    let result = Command::new("git")
        .args(args)
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}

fn repository() -> tempfile::TempDir {
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q"]);
    fs::create_dir_all(repo.path().join("context/pichi.build")).unwrap();
    fs::write(
        repo.path().join("context/pichi.build/carapace.yaml"),
        "from: base\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("context/pichi.build/refs.lock"),
        format!(
            "version: 1\nimages:\n  base:\n    index: sha256:{}\n",
            "0".repeat(64)
        ),
    )
    .unwrap();
    fs::write(repo.path().join("context/source"), "committed").unwrap();
    std::os::unix::fs::symlink("source", repo.path().join("context/link")).unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "fixture"]);
    repo
}

#[test]
fn capture_uses_commit_and_remains_independent_of_repository() {
    let repo = repository();
    let commit = git(repo.path(), &["rev-parse", "HEAD"]);
    fs::write(repo.path().join("context/source"), "uncommitted").unwrap();
    let context = Context::capture(repo.path(), &commit, Path::new("context")).unwrap();
    assert_eq!(context.commit, commit);
    context.pipeline().unwrap();
    drop(repo);
    let scratch = tempfile::tempdir().unwrap();
    let checkout = scratch.path().join("checkout");
    context.materialize(&checkout).unwrap();
    assert_eq!(fs::read(checkout.join("source")).unwrap(), b"committed");
    assert_eq!(
        fs::read_link(checkout.join("link")).unwrap(),
        Path::new("source")
    );
    assert!(context.materialize(&checkout).is_err());
}

#[test]
fn recipes_follow_only_committed_context_symlinks() {
    let repo = repository();
    let recipe = repo.path().join("context/pichi.build/carapace.yaml");
    fs::rename(&recipe, repo.path().join("context/base.yaml")).unwrap();
    std::os::unix::fs::symlink("../base.yaml", &recipe).unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "linked recipe"]);
    let commit = git(repo.path(), &["rev-parse", "HEAD"]);
    let context = Context::capture(repo.path(), &commit, Path::new("context")).unwrap();
    fs::write(repo.path().join("context/base.yaml"), "uncommitted garbage").unwrap();
    context.pipeline().unwrap();
    for target in [
        "../../outside.yaml",
        "/etc/passwd",
        "carapace.yaml",
        "missing",
    ] {
        fs::remove_file(&recipe).unwrap();
        std::os::unix::fs::symlink(target, &recipe).unwrap();
        git(repo.path(), &["add", "context/pichi.build/carapace.yaml"]);
        git(repo.path(), &["commit", "-qm", "invalid recipe link"]);
        let commit = git(repo.path(), &["rev-parse", "HEAD"]);
        assert!(
            Context::capture(repo.path(), &commit, Path::new("context"))
                .unwrap()
                .pipeline()
                .is_err()
        );
    }
}

#[test]
fn rejects_revision_expressions_context_escape_lfs_and_submodules() {
    let repo = repository();
    let commit = git(repo.path(), &["rev-parse", "HEAD"]);
    for revision in ["HEAD", &commit[..12]] {
        assert!(Context::capture(repo.path(), revision, Path::new("context")).is_err());
    }
    for path in ["../context", "/context", "context/source"] {
        assert!(Context::capture(repo.path(), &commit, Path::new(path)).is_err());
    }
    fs::write(
        repo.path().join("context/lfs"),
        "version https://git-lfs.github.com/spec/v1\noid sha256:abc\nsize 3\n",
    )
    .unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "lfs"]);
    let lfs_commit = git(repo.path(), &["rev-parse", "HEAD"]);
    assert!(
        Context::capture(repo.path(), &lfs_commit, Path::new("context"))
            .unwrap_err()
            .to_string()
            .contains("LFS")
    );
    git(repo.path(), &["rm", "-q", "context/lfs"]);
    git(
        repo.path(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},context/submodule"),
        ],
    );
    git(repo.path(), &["commit", "-qm", "submodule"]);
    let submodule_commit = git(repo.path(), &["rev-parse", "HEAD"]);
    assert!(
        Context::capture(repo.path(), &submodule_commit, Path::new("context"))
            .unwrap_err()
            .to_string()
            .contains("submodule")
    );
}

fn blob(root: &Path, bytes: &[u8]) -> String {
    let digest = format!("{:x}", Sha256::digest(bytes));
    fs::create_dir_all(root.join("blobs/sha256")).unwrap();
    fs::write(root.join("blobs/sha256").join(&digest), bytes).unwrap();
    format!("sha256:{digest}")
}

fn index(root: &Path, duplicate: bool) -> (IndexDigest, String) {
    let payload = b"firmware";
    let payload_digest = blob(root, payload);
    let manifest = json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.manifest.v1+json",
        "artifactType": "application/vnd.pichi.artifact.v1+json",
        "config": {"mediaType": "application/vnd.oci.empty.v1+json", "digest": "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a", "size": 2, "data": "e30="},
        "layers": [{"mediaType": "application/vnd.pichi.pmi.v1", "digest": payload_digest, "size": payload.len()}]
    });
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let manifest_digest = blob(root, &bytes);
    let descriptor = json!({"mediaType": "application/vnd.oci.image.manifest.v1+json", "artifactType": pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1, "digest": manifest_digest, "size": bytes.len(),
        "platform": {"os": "pichi", "architecture": if cfg!(target_arch="aarch64") {"arm64"} else {"amd64"}}});
    let descriptors = if duplicate {
        vec![descriptor.clone(), descriptor]
    } else {
        vec![descriptor]
    };
    let bytes = serde_json::to_vec(&json!({"schemaVersion": 2, "mediaType": "application/vnd.oci.image.index.v1+json", "manifests": descriptors})).unwrap();
    (
        IndexDigest::try_from(blob(root, &bytes)).unwrap(),
        payload_digest,
    )
}

#[test]
fn cache_verifies_index_manifest_and_every_payload() {
    let root = tempfile::tempdir().unwrap();
    let (locked, payload) = index(root.path(), false);
    let cache = Cache::new(root.path());
    let image = cache.resolve(&locked).unwrap();
    cache.verify_blobs(&image).unwrap();
    let path = root.path().join("blobs/sha256").join(&payload[7..]);
    fs::write(&path, "tampered").unwrap();
    assert!(cache.verify_blobs(&image).is_err());
    fs::remove_file(path).unwrap();
    assert!(cache.verify_blobs(&image).is_err());
    let manifest = IndexDigest::try_from(image.manifest_digest.clone()).unwrap();
    assert!(cache.resolve(&manifest).is_err());
    fs::write(
        root.path()
            .join("blobs/sha256")
            .join(&image.manifest_digest[7..]),
        "{}",
    )
    .unwrap();
    assert!(cache.resolve(&locked).is_err());
}

#[test]
fn ambiguous_platform_and_corrupt_index_are_errors() {
    let root = tempfile::tempdir().unwrap();
    let (locked, _) = index(root.path(), true);
    let cache = Cache::new(root.path());
    assert!(
        cache
            .resolve(&locked)
            .unwrap_err()
            .to_string()
            .contains("ambiguous")
    );
    fs::write(
        root.path().join("blobs/sha256").join(&locked.as_ref()[7..]),
        "{}",
    )
    .unwrap();
    assert!(cache.resolve(&locked).is_err());
    let (locked, _) = index(root.path(), false);
    let bytes = fs::read(root.path().join("blobs/sha256").join(&locked.as_ref()[7..])).unwrap();
    let mut foreign: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    foreign["manifests"][0]["platform"]["os"] = "linux".into();
    let foreign =
        IndexDigest::try_from(blob(root.path(), &serde_json::to_vec(&foreign).unwrap())).unwrap();
    assert!(
        cache
            .resolve(&foreign)
            .unwrap_err()
            .to_string()
            .contains("no compatible host platform")
    );
    let mut mixed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut other = mixed["manifests"][0].clone();
    other["artifactType"] = "application/vnd.example.other".into();
    mixed["manifests"].as_array_mut().unwrap().push(other);
    let mixed =
        IndexDigest::try_from(blob(root.path(), &serde_json::to_vec(&mixed).unwrap())).unwrap();
    cache.resolve(&mixed).unwrap();
}

#[test]
fn sha256_git_objects_are_verified_and_recorded() {
    let repo = tempfile::tempdir().unwrap();
    git(repo.path(), &["init", "-q", "--object-format=sha256"]);
    fs::write(repo.path().join("file"), "contents").unwrap();
    git(repo.path(), &["add", "."]);
    git(repo.path(), &["commit", "-qm", "sha256"]);
    let commit = git(repo.path(), &["rev-parse", "HEAD"]);
    let context = Context::capture(repo.path(), &commit, Path::new(".")).unwrap();
    assert_eq!(context.object_format, "sha256");
    assert_eq!(context.commit.len(), 64);
    assert_eq!(context.tree.len(), 64);
    // Mutating a loose object under its original identity must not be accepted.
    let other = git(repo.path(), &["hash-object", "-w", "file"]);
    let original_path = repo
        .path()
        .join(".git/objects")
        .join(&commit[..2])
        .join(&commit[2..]);
    let other_path = repo
        .path()
        .join(".git/objects")
        .join(&other[..2])
        .join(&other[2..]);
    fs::remove_file(&original_path).unwrap();
    fs::copy(other_path, original_path).unwrap();
    assert!(Context::capture(repo.path(), &commit, Path::new(".")).is_err());
}

#[test]
fn input_resolution_connects_committed_lock_to_execution_images() {
    let repo = repository();
    let store = tempfile::tempdir().unwrap();
    let (locked, payload) = index(store.path(), false);
    let cache = Cache::new(store.path());
    let commit_lock = |digest: &str| {
        fs::write(
            repo.path().join("context/pichi.build/refs.lock"),
            format!("version: 1\nimages:\n  base:\n    index: {digest}\n"),
        )
        .unwrap();
        git(repo.path(), &["add", "."]);
        git(repo.path(), &["commit", "-qm", "lock"]);
        git(repo.path(), &["rev-parse", "HEAD"])
    };
    let commit = commit_lock(locked.as_ref());
    let error =
        conglobate::input::Inputs::resolve(repo.path(), &commit, Path::new("context"), &cache)
            .unwrap_err();
    assert!(error.to_string().contains("no carapace"));

    let resolved = cache.resolve(&locked).unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&resolved.manifest_bytes).unwrap();
    manifest["layers"][0] = json!({"mediaType": "application/vnd.pichi.scute.v1", "digest": payload, "size": 8,
        "annotations": {"dev.pichi.scute.verity.salt": "00"}});
    manifest["annotations"] = json!({
        "dev.pichi.carapace.verity.algo": "sha256",
        "dev.pichi.carapace.verity.data-block-size": "4096",
        "dev.pichi.carapace.verity.hash-block-size": "4096",
        "dev.pichi.carapace.verity.version": "1",
        "dev.pichi.carapace.verity.hash-type": "1",
        "dev.pichi.carapace.verity.hash": "ab".repeat(32)
    });
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let digest = blob(store.path(), &bytes);
    let mut index: serde_json::Value = serde_json::from_slice(
        &fs::read(
            store
                .path()
                .join("blobs/sha256")
                .join(&locked.as_ref()[7..]),
        )
        .unwrap(),
    )
    .unwrap();
    index["manifests"][0]["digest"] = digest.into();
    index["manifests"][0]["size"] = bytes.len().into();
    let digest = blob(store.path(), &serde_json::to_vec(&index).unwrap());
    let commit = commit_lock(&digest);
    let inputs =
        conglobate::input::Inputs::resolve(repo.path(), &commit, Path::new("context"), &cache)
            .unwrap();
    assert_eq!(inputs.images.len(), 1);
    assert_eq!(
        inputs
            .images
            .values()
            .next()
            .unwrap()
            .manifest
            .carapace_verity_hash(),
        Some([0xab; 32])
    );
    let output = &inputs.images.values().next().unwrap().manifest;
    let order = vec![output.layers[0].digest_str().to_owned()];
    let record = conglobate::inventory::Record {
        inputs: &inputs,
        output,
        repository: repo.path(),
        subdirectory: Path::new("context"),
        before_transform: None,
        scute_order: &order,
    }
    .inventory()
    .unwrap();
    assert_eq!(record.inputs[0].roles, ["carapace-base"]);
    assert_eq!(record.outputs.scutes[0].origin, "inherited");
    assert!(record.execution.transform.default_skipped);
    assert!(record.execution.recipes.contains_key("refs.lock"));
    conglobate::inventory::Inventory::decode(&record.encode().unwrap()).unwrap();
    // Preflight verifies the blob identity, not the dm-verity chain. The
    // executor must establish that separately before mounting this image.
}
