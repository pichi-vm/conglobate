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
    use conglobate::path::ContextPath;
    for format in ["sha1", "sha256"] {
        let repo = Repository::new(format);
        repo.write("build/plan.yml", b"committed");
        repo.write("outside", b"not selected");
        let commit = repo.commit();
        repo.write("build/plan.yml", b"working tree modified");
        let capture = repo.capture(&commit, "build").unwrap();
        assert_eq!(capture.entries().len(), 1);
        assert!(matches!(
            capture.entries().get(Path::new("plan.yml")),
            Some(ContextEntry::File { .. })
        ));
        let path = ContextPath::try_from(Path::new("plan.yml")).unwrap();
        assert_eq!(capture.read(&path).unwrap().bytes(), b"committed");
        assert_eq!(
            capture.subtree().to_string(),
            repo.git(&["rev-parse", "HEAD:build"])
        );
        assert_eq!(capture.commit().as_bytes(), commit.bytes());
    }
}

#[test]
fn tampered_blob_bytes_fail_identity_verification() {
    use conglobate::path::ContextPath;
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
    let capture = repo.capture(&commit, ".").unwrap();
    let path = ContextPath::try_from(Path::new("a")).unwrap();
    let error = capture.read(&path).unwrap_err().to_string();
    assert!(error.contains("identity"), "{error}");
}

#[test]
fn unsupported_lfs_and_submodules_fail() {
    use conglobate::path::ContextPath;
    let repo = Repository::new("sha1");
    repo.write(
        "pointer",
        b"version https://git-lfs.github.com/spec/v1\noid sha256:123\nsize 1\n",
    );
    let commit = repo.commit();
    let capture = repo.capture(&commit, ".").unwrap();
    let path = ContextPath::try_from(Path::new("pointer")).unwrap();
    assert!(capture.read(&path).unwrap_err().to_string().contains("LFS"));
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
    repo.write("dir/plan", b"selected");
    for (name, target) in [
        ("selected", "dir/plan"),
        ("escape", "../outside"),
        ("absolute", "/etc/passwd"),
        ("loop", "loop"),
        ("broken", "missing/../dir/plan"),
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
fn capture_defers_regular_blob_loads_until_read() {
    use conglobate::path::ContextPath;
    let repo = Repository::new("sha1");
    repo.write("kept", b"keep-me");
    repo.write("unused", b"never-read");
    let commit = repo.commit();
    let unused = repo.git(&["rev-parse", "HEAD:unused"]);
    let object_path = repo
        .0
        .path()
        .join(".git/objects")
        .join(&unused[..2])
        .join(&unused[2..]);
    // Remove an unused blob before capture; metadata walk must still succeed.
    std::fs::remove_file(&object_path).unwrap();
    let capture = repo.capture(&commit, ".").unwrap();
    let kept = ContextPath::try_from(Path::new("kept")).unwrap();
    assert_eq!(capture.read(&kept).unwrap().bytes(), b"keep-me");
    let unused_path = ContextPath::try_from(Path::new("unused")).unwrap();
    assert!(capture.read(&unused_path).is_err());
}

#[test]
fn packed_objects_are_supported_and_capture_does_not_execute_filters() {
    let repo = Repository::new("sha1");
    repo.write(".gitattributes", b"* filter=fixture\n");
    repo.write("plan", b"original\n");
    let commit = repo.commit();
    // A failing filter is enough to detect accidental execution; no external effect.
    repo.git(&["config", "filter.fixture.smudge", "false"]);
    repo.git(&["config", "filter.fixture.required", "true"]);
    repo.git(&["gc", "--prune=now"]);
    let capture = repo.capture(&commit, ".").unwrap();
    let path = conglobate::path::ContextPath::try_from(Path::new("plan")).unwrap();
    assert_eq!(capture.read(&path).unwrap().bytes(), b"original\n");
}

#[test]
fn request_planning_and_document_receipts_use_only_selected_committed_bytes() {
    use conglobate::{input::CapturedBuild, request::BuildRequest};
    use sha2::{Digest, Sha256};
    let repo = Repository::new("sha1");
    let plan =
        b"tasks:\n  rootfs:\n    kind: extend\n    from: registry.test/base:1\nimage: rootfs\n";
    let lock = format!(
        "version: 1\nimages:\n  registry.test/base:1:\n    index: sha256:{}\n",
        "11".repeat(32)
    );
    repo.write("context/config/build.yml", plan);
    repo.write("context/pins.yml", lock.as_bytes());
    std::os::unix::fs::symlink(
        "config/build.yml",
        repo.0.path().join("context/selected.yml"),
    )
    .unwrap();
    let commit = repo.commit();
    repo.write("context/config/build.yml", b"not the plan");
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
        "--plan",
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
        build.plan_document().path().as_ref(),
        Path::new("selected.yml")
    );
    assert_eq!(
        build.plan_document().sha256().as_slice(),
        Sha256::digest(plan).as_slice()
    );
    assert_eq!(
        build.lock_document().sha256().as_slice(),
        Sha256::digest(lock.as_bytes()).as_slice()
    );
    assert_eq!(
        build.graph().inputs()[0].reference().as_ref(),
        "registry.test/base:1"
    );
}

#[test]
fn network_plan_captures_an_explicit_resolver_once() {
    use conglobate::{
        input::{CapturedBuild, InputError},
        request::BuildRequest,
    };
    let repo = Repository::new("sha1");
    repo.write(
        "plan",
        b"tasks:\n  rootfs:\n    kind: extend\n    from: registry.test/base:1\n    exec:\n    - run: echo ok\n      network: true\nimage: rootfs\n",
    );
    repo.write(
        "lock",
        format!(
            "version: 1\nimages:\n  registry.test/base:1:\n    index: sha256:{}\n",
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
        "--plan",
        "plan",
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

    // An executing offline graph still needs environment-allocated identities.
    repo.write(
        "plan",
        b"tasks:\n  rootfs:\n    kind: extend\n    from: registry.test/base:1\n    exec:\n    - run: echo ok\nimage: rootfs\n",
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
    repo.write(
        "plan",
        b"tasks:\n  rootfs:\n    kind: extend\n    from: registry.test/base:1\nimage: rootfs\n",
    );
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
fn resolved_extend_inputs_require_carapaces_with_or_without_commands() {
    use crate::artifact::Digest;
    use conglobate::{
        cache::{Cache, HostArchitecture},
        input::{CapturedBuild, InputError},
        request::BuildRequest,
    };
    let manifest = crate::artifact::Manifest {
        schema_version: 2,
        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
        artifact_type: crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
        config: crate::artifact::ConfigDescriptor::canonical(),
        layers: Vec::new(),
        annotations: Default::default(),
    }
    .to_bytes()
    .unwrap();
    let manifest_digest = Digest::from_bytes_sha256(&manifest);
    let index = serde_json::to_vec(&serde_json::json!({"schemaVersion":2,
        "mediaType":"application/vnd.oci.image.index.v1+json", "manifests":[{
            "mediaType":"application/vnd.oci.image.manifest.v1+json", "artifactType":crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
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
        format!("version: 1\nimages:\n  registry.test/base:1:\n    index: {index_digest}\n")
            .as_bytes(),
    );
    for plan in [
        "tasks: {root: {kind: extend, from: registry.test/base:1}}\nimage: root",
        "tasks: {root: {kind: extend, from: registry.test/base:1, exec: [{run: echo ok}]}}\nimage: root",
    ] {
        repo.write("plan", plan.as_bytes());
        let commit = repo.commit();
        let request = BuildRequest::parse_from([
            "conglobate",
            "build",
            "--repository",
            repo.0.path().to_str().unwrap(),
            "--commit",
            &hex::encode(commit.bytes()),
            "--plan",
            "plan",
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
        assert!(matches!(
            result,
            Err(InputError::MissingExecutionCarapace(_))
        ));
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
    let mut with_dtb: crate::artifact::Manifest =
        serde_json::from_slice(&std::fs::read(blobs.join(manifest_digest.hex())).unwrap()).unwrap();
    with_dtb.layers.push(crate::artifact::Layer::Dtb(
        crate::artifact::DtbDescriptor {
            digest: dtb_digest.to_string(),
            size: dtb.len() as u64,
        },
    ));
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
        format!("version: 1\nimages:\n  registry.test/base:1:\n    index: {pin}\n").as_bytes(),
    );
    repo.write(
        "plan",
        b"tasks:\n  rootfs:\n    kind: extend\n    from: registry.test/base:1\nimage: rootfs\n",
    );
    let commit = repo.commit();
    let request = BuildRequest::parse_from([
        "conglobate",
        "build",
        "--repository",
        repo.0.path().to_str().unwrap(),
        "--commit",
        &hex::encode(commit.bytes()),
        "--plan",
        "plan",
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
    assert!(matches!(
        CapturedBuild::try_from(&request)
            .unwrap()
            .resolve(Cache::open(cache.path()).unwrap()),
        Err(InputError::MissingExecutionCarapace(_))
    ));
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

#[test]
fn executing_stage_variants_require_environment_maps_before_cache_access() {
    use conglobate::{
        input::{CapturedBuild, InputError},
        request::BuildRequest,
    };
    let repo = Repository::new("sha1");
    repo.write(
        "lock",
        format!(
            "version: 1\nimages:\n  registry.test/tools:1: {{index: sha256:{}}}\n",
            "11".repeat(32),
        )
        .as_bytes(),
    );
    let cache = repo.0.path().join("missing-cache");
    let output = repo.0.path().join("unpublished-output");
    for plan in [
        "tasks:\n  bootstrap:\n    kind: bootstrap\n    from: registry.test/tools:1\n    filesystem:\n      size: 16MiB\n      kind: ext4\nimage: bootstrap\n",
        "tasks:\n  rootfs:\n    kind: extend\n    from: registry.test/tools:1\n    exec:\n    - run: 'true'\nimage: rootfs\n",
        "tasks:\n  rootfs:\n    kind: extend\n    from: registry.test/tools:1\n    exec:\n    - copy:\n        from: input\n        into: /input\nimage: rootfs\n",
        "tasks:\n  firmware:\n    kind: build\n    from: registry.test/tools:1\n    into:\n      pmi: /pmi\n      dtb: /dtb\nimage: firmware\n",
        "tasks:\n  rootfs:\n    kind: extend\n    from: registry.test/tools:1\n  firmware:\n    kind: build\n    from: rootfs\n    into:\n      pmi: /pmi\n      dtb: /dtb\nimage:\n  root: rootfs\n  boot: firmware\n",
    ] {
        repo.write("plan", plan.as_bytes());
        repo.write("input", b"committed bytes");
        let commit = repo.commit();
        let identity = hex::encode(commit.bytes());
        let mut args = vec![
            "conglobate",
            "build",
            "--repository",
            repo.0.path().to_str().unwrap(),
            "--commit",
            &identity,
            "--plan",
            "plan",
            "--lock",
            "lock",
            "--cache",
            cache.to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ];
        let request = BuildRequest::parse_from(&args).unwrap();
        assert!(
            matches!(
                CapturedBuild::try_from(&request),
                Err(InputError::IdMappingsRequired)
            ),
            "{plan}"
        );
        args.extend(["--uid-map", "200000:131072", "--gid-map", "400000:65536"]);
        let captured = CapturedBuild::try_from(&BuildRequest::parse_from(&args).unwrap()).unwrap();
        let maps = captured.id_mappings().unwrap();
        assert_eq!(maps.uid().count(), 131072);
        assert_eq!(maps.gid().count(), 65536);
        assert!(!cache.exists());
        assert!(!output.exists());
    }
}
