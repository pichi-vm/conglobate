// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    context::{CapturedContext, ContextEntry},
    path::ContextDirectory,
    request::CommitIdentity,
};
use std::{path::Path, process::Command};

struct Repository(tempfile::TempDir);
impl Repository {
    fn new(format: &str) -> Self {
        let repo = Self(tempfile::tempdir().unwrap());
        repo.git(&["init", &format!("--object-format={format}")]);
        repo.git(&["config", "user.name", "Fixture"]);
        repo.git(&["config", "user.email", "fixture@example.invalid"]);
        repo
    }
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.0.path())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
    fn write(&self, name: &str, bytes: &[u8]) {
        let path = self.0.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
    fn commit(&self) -> CommitIdentity {
        self.git(&["add", "."]);
        self.git(&["commit", "-m", "fixture"]);
        CommitIdentity::try_from(self.git(&["rev-parse", "HEAD"]).as_str()).unwrap()
    }
    fn capture(
        &self,
        commit: &CommitIdentity,
        context: &str,
    ) -> Result<CapturedContext, conglobate::context::ContextError> {
        CapturedContext::capture(
            self.0.path(),
            commit,
            &ContextDirectory::try_from(Path::new(context)).unwrap(),
        )
    }
}

#[test]
fn capture_uses_verified_commit_and_subtree_in_both_object_formats() {
    for format in ["sha1", "sha256"] {
        let repo = Repository::new(format);
        repo.write("build/recipe.yml", b"committed");
        repo.write("outside", b"not selected");
        let commit = repo.commit();
        repo.write("build/recipe.yml", b"working tree modified");
        let capture = repo.capture(&commit, "build").unwrap();
        assert_eq!(capture.entries().len(), 1);
        let ContextEntry::File { bytes, .. } = &capture.entries()[Path::new("recipe.yml")] else {
            panic!("not a file")
        };
        assert_eq!(bytes, b"committed");
        assert_eq!(
            capture.subtree().to_string(),
            repo.git(&["rev-parse", "HEAD:build"])
        );
        assert_eq!(capture.commit().as_bytes(), commit.bytes());
        std::fs::remove_dir_all(repo.0.path().join(".git")).unwrap();
        assert_eq!(bytes, b"committed");
    }
}

#[test]
fn tampered_blob_bytes_fail_identity_verification() {
    let repo = Repository::new("sha1");
    repo.write("a", b"original");
    repo.write("b", b"different");
    let commit = repo.commit();
    let original = repo.git(&["rev-parse", "HEAD:a"]);
    let other = repo.git(&["rev-parse", "HEAD:b"]);
    let object_path = |id: &str| {
        repo.0
            .path()
            .join(".git/objects")
            .join(&id[..2])
            .join(&id[2..])
    };
    std::fs::remove_file(object_path(&original)).unwrap();
    std::fs::copy(object_path(&other), object_path(&original)).unwrap();
    let error = repo.capture(&commit, ".").unwrap_err().to_string();
    assert!(error.contains("identity"), "{error}");
}

#[test]
fn unsupported_lfs_and_submodules_fail() {
    let repo = Repository::new("sha1");
    repo.write(
        "pointer",
        b"version https://git-lfs.github.com/spec/v1\noid sha256:123\nsize 1\n",
    );
    let commit = repo.commit();
    assert!(
        repo.capture(&commit, ".")
            .unwrap_err()
            .to_string()
            .contains("LFS")
    );
    repo.git(&["rm", "pointer"]);
    let id = hex::encode(commit.bytes());
    repo.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{id},module"),
    ]);
    repo.git(&["commit", "-m", "gitlink"]);
    let commit = CommitIdentity::try_from(repo.git(&["rev-parse", "HEAD"]).as_str()).unwrap();
    assert!(
        repo.capture(&commit, ".")
            .unwrap_err()
            .to_string()
            .contains("submodule")
    );
}

#[test]
fn selected_files_follow_only_confined_committed_links() {
    use conglobate::path::ContextPath;
    let repo = Repository::new("sha1");
    repo.write("dir/recipe", b"selected");
    for (name, target) in [
        ("selected", "dir/recipe"),
        ("escape", "../outside"),
        ("absolute", "/etc/passwd"),
        ("loop", "loop"),
        ("broken", "missing/../dir/recipe"),
    ] {
        std::os::unix::fs::symlink(target, repo.0.path().join(name)).unwrap();
    }
    let commit = repo.commit();
    let capture = repo.capture(&commit, ".").unwrap();
    let selected = ContextPath::try_from(Path::new("selected")).unwrap();
    assert_eq!(capture.read(&selected).unwrap().bytes(), b"selected");
    for name in ["escape", "absolute", "loop", "broken", "dir"] {
        let path = ContextPath::try_from(Path::new(name)).unwrap();
        assert!(capture.read(&path).is_err(), "{name}");
    }
}

#[test]
fn packed_objects_are_supported_and_capture_does_not_execute_filters() {
    let repo = Repository::new("sha1");
    repo.write(".gitattributes", b"* filter=fixture\n");
    repo.write("recipe", b"original\n");
    let commit = repo.commit();
    // A failing filter is enough to detect accidental execution; no external effect.
    repo.git(&["config", "filter.fixture.smudge", "false"]);
    repo.git(&["config", "filter.fixture.required", "true"]);
    repo.git(&["gc", "--prune=now"]);
    let capture = repo.capture(&commit, ".").unwrap();
    let path = conglobate::path::ContextPath::try_from(Path::new("recipe")).unwrap();
    assert_eq!(capture.read(&path).unwrap().bytes(), b"original\n");
}

#[test]
fn request_planning_and_document_receipts_use_only_selected_committed_bytes() {
    use conglobate::{input::CapturedBuild, request::BuildRequest};
    use sha2::{Digest, Sha256};
    let repo = Repository::new("sha1");
    let recipe = b"carapace:\n  from: base\n";
    let lock = format!(
        "version: 1\nimages:\n  base:\n    index: sha256:{}\n",
        "11".repeat(32)
    );
    repo.write("context/config/build.yml", recipe);
    repo.write("context/pins.yml", lock.as_bytes());
    std::os::unix::fs::symlink(
        "config/build.yml",
        repo.0.path().join("context/selected.yml"),
    )
    .unwrap();
    let commit = repo.commit();
    repo.write("context/config/build.yml", b"not the recipe");
    repo.write("context/pins.yml", b"not the lock");
    let request = BuildRequest::parse_from([
        "conglobate",
        "build",
        "--repository",
        repo.0.path().to_str().unwrap(),
        "--commit",
        &hex::encode(commit.bytes()),
        "--context",
        "context",
        "--recipe",
        "selected.yml",
        "--lock",
        "pins.yml",
        "--cache",
        "/unused-cache",
        "--output",
        "/unused-output",
        "--uid-map",
        "200000:131072",
        "--gid-map",
        "400000:131072",
    ])
    .unwrap();
    let build = CapturedBuild::try_from(&request).unwrap();
    assert_eq!(
        build.recipe_document().path().as_ref(),
        Path::new("selected.yml")
    );
    assert_eq!(
        build.recipe_document().sha256().as_slice(),
        Sha256::digest(recipe).as_slice()
    );
    assert_eq!(
        build.lock_document().sha256().as_slice(),
        Sha256::digest(lock.as_bytes()).as_slice()
    );
    assert_eq!(build.plan().inputs()[0].reference().as_ref(), "base");
}

#[test]
fn network_recipe_captures_an_explicit_resolver_once() {
    use conglobate::{
        input::{CapturedBuild, InputError},
        request::BuildRequest,
    };
    let repo = Repository::new("sha1");
    repo.write(
        "recipe",
        b"carapace:\n  from: base\n  derive:\n    - run: echo ok\n      network: true\n",
    );
    repo.write(
        "lock",
        format!(
            "version: 1\nimages:\n  base:\n    index: sha256:{}\n",
            "11".repeat(32)
        )
        .as_bytes(),
    );
    let commit = repo.commit();
    let id = hex::encode(commit.bytes());
    let mut args = vec![
        "conglobate",
        "build",
        "--repository",
        repo.0.path().to_str().unwrap(),
        "--commit",
        &id,
        "--recipe",
        "recipe",
        "--lock",
        "lock",
        "--cache",
        "/unused-cache",
        "--output",
        "/unused-output",
        "--uid-map",
        "200000:131072",
        "--gid-map",
        "400000:131072",
    ];
    let request = BuildRequest::parse_from(&args).unwrap();
    assert!(matches!(
        CapturedBuild::try_from(&request),
        Err(InputError::ResolverRequired)
    ));
    let resolver = tempfile::NamedTempFile::new().unwrap();
    let selected = b"nameserver 192.0.2.53\n";
    std::fs::write(resolver.path(), selected).unwrap();
    args.extend(["--resolv-conf", resolver.path().to_str().unwrap()]);
    let request = BuildRequest::parse_from(&args).unwrap();
    assert!(matches!(
        CapturedBuild::try_from(&request),
        Err(InputError::NetworkNamespaceRequired)
    ));
    args.extend(["--network-namespace", "/proc/self/ns/net"]);
    let request = BuildRequest::parse_from(&args).unwrap();
    assert!(matches!(
        CapturedBuild::try_from(&request),
        Err(InputError::NetworkNamespace(_))
    ));
    std::fs::remove_file(resolver.path()).unwrap();
    assert!(matches!(
        CapturedBuild::try_from(&request),
        Err(InputError::Resolver(_))
    ));

    // An executing offline plan still needs environment-allocated identities.
    repo.write(
        "recipe",
        b"carapace:\n  from: base\n  derive:\n    - run: echo ok\n",
    );
    let execution_commit = repo.commit();
    let execution_id = hex::encode(execution_commit.bytes());
    let position = args.iter().position(|arg| *arg == "--commit").unwrap();
    args[position + 1] = &execution_id;
    let mut without_maps = args.clone();
    for flag in ["--uid-map", "--gid-map"] {
        let position = without_maps.iter().position(|arg| *arg == flag).unwrap();
        without_maps.drain(position..position + 2);
    }
    let request = BuildRequest::parse_from(&without_maps).unwrap();
    assert!(matches!(
        CapturedBuild::try_from(&request),
        Err(InputError::IdMappingsRequired)
    ));
    let request = BuildRequest::parse_from(&args).unwrap();
    let captured = CapturedBuild::try_from(&request).unwrap();
    assert_eq!(captured.id_mappings().unwrap().uid().count(), 131072);

    // Pass-through ignores supplied runtime inputs, including mappings.
    repo.write("recipe", b"carapace: {from: base}\n");
    let offline_commit = repo.commit();
    let offline_id = hex::encode(offline_commit.bytes());
    let position = args.iter().position(|arg| *arg == "--commit").unwrap();
    args[position + 1] = &offline_id;
    let request = BuildRequest::parse_from(&args).unwrap();
    let captured = CapturedBuild::try_from(&request).unwrap();
    assert!(captured.networking().is_none());
    assert!(captured.id_mappings().is_none());
    let position = without_maps
        .iter()
        .position(|arg| *arg == "--commit")
        .unwrap();
    without_maps[position + 1] = &offline_id;
    let request = BuildRequest::parse_from(&without_maps).unwrap();
    assert!(
        CapturedBuild::try_from(&request)
            .unwrap()
            .id_mappings()
            .is_none()
    );
}

#[test]
fn resolved_inputs_allow_firmware_passthrough_but_require_a_carapace_for_commands() {
    use conglobate::{
        cache::{Cache, HostArchitecture},
        input::{CapturedBuild, InputError},
        request::BuildRequest,
    };
    use pichi_artifact::Digest;
    let manifest = pichi_artifact::Manifest {
        schema_version: 2,
        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
        artifact_type: pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
        config: pichi_artifact::ConfigDescriptor::canonical(),
        layers: Vec::new(),
        annotations: Default::default(),
    }
    .to_bytes()
    .unwrap();
    let manifest_digest = Digest::from_bytes_sha256(&manifest);
    let index = serde_json::to_vec(&serde_json::json!({"schemaVersion":2,
        "mediaType":"application/vnd.oci.image.index.v1+json", "manifests":[{
            "mediaType":"application/vnd.oci.image.manifest.v1+json", "artifactType":pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
            "digest":manifest_digest.to_string(),"size":manifest.len(),
            "platform":{"os":"pichi","architecture":HostArchitecture::current().unwrap().oci_name()}}]})).unwrap();
    let index_digest = Digest::from_bytes_sha256(&index);
    let cache = tempfile::tempdir().unwrap();
    let blobs = cache.path().join("blobs/sha256");
    std::fs::create_dir_all(&blobs).unwrap();
    std::fs::write(blobs.join(manifest_digest.hex()), manifest).unwrap();
    std::fs::write(blobs.join(index_digest.hex()), index).unwrap();
    let repo = Repository::new("sha1");
    repo.write(
        "lock",
        format!("version: 1\nimages:\n  base:\n    index: {index_digest}\n").as_bytes(),
    );
    for (recipe, execution) in [
        ("carapace: {from: base}", false),
        ("carapace: {from: base, derive: [{run: echo ok}]}", true),
        ("carapace: {from: base}\ntransform: []", false),
        (
            "carapace: {from: base}\ntransform: [{op: remove, property: /chosen/bootargs}]",
            false,
        ),
    ] {
        repo.write("recipe", recipe.as_bytes());
        let commit = repo.commit();
        let request = BuildRequest::parse_from([
            "conglobate",
            "build",
            "--repository",
            repo.0.path().to_str().unwrap(),
            "--commit",
            &hex::encode(commit.bytes()),
            "--recipe",
            "recipe",
            "--lock",
            "lock",
            "--cache",
            cache.path().to_str().unwrap(),
            "--output",
            "/unused-output",
            "--uid-map",
            "200000:131072",
            "--gid-map",
            "400000:131072",
        ])
        .unwrap();
        let result = CapturedBuild::try_from(&request)
            .unwrap()
            .resolve(Cache::open(cache.path()).unwrap());
        if execution {
            assert!(matches!(
                result,
                Err(InputError::MissingExecutionCarapace(_))
            ));
        } else if recipe.contains("transform:") {
            assert!(matches!(result, Err(InputError::MissingTransformDtb)));
        } else {
            assert_eq!(result.unwrap().images().len(), 1);
        }
    }
    // A structurally valid empty DTB supplies the inherited component. This
    // preflight checks presence/content identity; transformation parses it later.
    let mut dtb = Vec::new();
    for word in [0xd00dfeed_u32, 72, 56, 72, 40, 17, 16, 0, 0, 16] {
        dtb.extend(word.to_be_bytes());
    }
    dtb.extend([0; 16]);
    for word in [1_u32, 0, 2, 9] {
        dtb.extend(word.to_be_bytes());
    }
    let dtb_digest = Digest::from_bytes_sha256(&dtb);
    std::fs::write(blobs.join(dtb_digest.hex()), &dtb).unwrap();
    let mut with_dtb: pichi_artifact::Manifest =
        serde_json::from_slice(&std::fs::read(blobs.join(manifest_digest.hex())).unwrap()).unwrap();
    with_dtb
        .layers
        .push(pichi_artifact::Layer::Dtb(pichi_artifact::DtbDescriptor {
            digest: dtb_digest.to_string(),
            size: dtb.len() as u64,
        }));
    let bytes = with_dtb.to_bytes().unwrap();
    let digest = Digest::from_bytes_sha256(&bytes);
    std::fs::write(blobs.join(digest.hex()), &bytes).unwrap();
    let mut with_dtb_index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(blobs.join(index_digest.hex())).unwrap()).unwrap();
    with_dtb_index["manifests"][0]["digest"] = digest.to_string().into();
    with_dtb_index["manifests"][0]["size"] = bytes.len().into();
    let index = serde_json::to_vec(&with_dtb_index).unwrap();
    let pin = Digest::from_bytes_sha256(&index);
    std::fs::write(blobs.join(pin.hex()), index).unwrap();
    repo.write(
        "lock",
        format!("version: 1\nimages:\n  base:\n    index: {pin}\n").as_bytes(),
    );
    repo.write("recipe", b"carapace: {from: base}\ntransform: []");
    let commit = repo.commit();
    let request = BuildRequest::parse_from([
        "conglobate",
        "build",
        "--repository",
        repo.0.path().to_str().unwrap(),
        "--commit",
        &hex::encode(commit.bytes()),
        "--recipe",
        "recipe",
        "--lock",
        "lock",
        "--cache",
        cache.path().to_str().unwrap(),
        "--output",
        "/unused-output",
        "--uid-map",
        "200000:131072",
        "--gid-map",
        "400000:131072",
    ])
    .unwrap();
    assert!(
        CapturedBuild::try_from(&request)
            .unwrap()
            .resolve(Cache::open(cache.path()).unwrap())
            .is_ok()
    );
}

#[test]
fn captured_bytes_modes_and_links_survive_source_changes() {
    use conglobate::path::ContextPath;
    use std::os::unix::fs::PermissionsExt;
    let repo = Repository::new("sha1");
    repo.write("dir/plain", b"original");
    repo.write("dir/executable", b"#!/bin/sh\n");
    std::fs::set_permissions(
        repo.0.path().join("dir/executable"),
        std::fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    std::os::unix::fs::symlink("dir/plain", repo.0.path().join("link")).unwrap();
    std::os::unix::fs::symlink("../outside", repo.0.path().join("escape")).unwrap();
    let commit = repo.commit();
    let captured = repo.capture(&commit, ".").unwrap();
    repo.write("dir/plain", b"changed");
    let plain = captured
        .read(&ContextPath::try_from(Path::new("link")).unwrap())
        .unwrap();
    assert_eq!(plain.bytes(), b"original");
    assert!(!plain.executable());
    assert!(
        captured
            .read(&ContextPath::try_from(Path::new("dir/executable")).unwrap())
            .unwrap()
            .executable()
    );
    assert!(
        captured
            .read(&ContextPath::try_from(Path::new("escape")).unwrap())
            .is_err()
    );
}

impl Repository {
    fn object(&self, kind: &str, bytes: &[u8]) -> String {
        use std::{io::Write, process::Stdio};
        let mut child = Command::new("git")
            .args(["hash-object", "--literally", "-w", "--stdin", "-t", kind])
            .current_dir(self.0.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }
}
#[test]
fn subtree_selection_rejects_duplicate_entries_in_ancestor_trees() {
    let repo = Repository::new("sha1");
    let empty = repo.object("tree", b"");
    let mut tree = Vec::new();
    for _ in 0..2 {
        tree.extend_from_slice(b"40000 selected\0");
        tree.extend(hex::decode(&empty).unwrap());
    }
    let tree = repo.object("tree", &tree);
    let commit = repo.object("commit", format!("tree {tree}\nauthor Fixture <fixture@example.invalid> 1 +0000\ncommitter Fixture <fixture@example.invalid> 1 +0000\n\nfixture\n").as_bytes());
    let commit = CommitIdentity::try_from(commit.as_str()).unwrap();
    assert!(
        repo.capture(&commit, "selected")
            .unwrap_err()
            .to_string()
            .contains("duplicate")
    );
}
#[test]
fn captured_reads_reject_trailing_symlink_separators() {
    use conglobate::path::ContextPath;
    let repo = Repository::new("sha1");
    repo.write("file", b"content");
    std::os::unix::fs::symlink("file/", repo.0.path().join("link")).unwrap();
    std::os::unix::fs::symlink("file/.", repo.0.path().join("dotlink")).unwrap();
    let commit = repo.commit();
    let captured = repo.capture(&commit, ".").unwrap();
    let path = ContextPath::try_from(Path::new("link")).unwrap();
    let dotpath = ContextPath::try_from(Path::new("dotlink")).unwrap();
    assert!(captured.read(&path).is_err());
    assert!(captured.read(&dotpath).is_err());
}
