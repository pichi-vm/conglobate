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
        self.git(&["add", "--ignore-removal", "."]);
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
        let mut capture = repo.capture(&commit, "build").unwrap();
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
    let mut capture = repo.capture(&commit, ".").unwrap();
    let path = ContextPath::try_from(Path::new("a")).unwrap();
    let error = capture.read(&path).unwrap_err().to_string();
    assert!(error.contains("identity"), "{error}");
}

#[test]
fn lfs_fails_on_use_and_unused_gitlinks_are_ignored() {
    use conglobate::path::ContextPath;
    let repo = Repository::new("sha1");
    repo.write(
        "pointer",
        b"version https://git-lfs.github.com/spec/v1\noid sha256:123\nsize 1\n",
    );
    let commit = repo.commit();
    let mut capture = repo.capture(&commit, ".").unwrap();
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
    let mut capture = repo.capture(&commit, ".").unwrap();
    let path = ContextPath::try_from(Path::new("module/file")).unwrap();
    assert!(
        capture
            .read(&path)
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
    let mut capture = repo.capture(&commit, ".").unwrap();
    let selected = ContextPath::try_from(Path::new("selected")).unwrap();
    assert_eq!(capture.read(&selected).unwrap().bytes(), b"selected");
    for name in ["escape", "absolute", "loop", "broken", "dir"] {
        let path = ContextPath::try_from(Path::new(name)).unwrap();
        assert!(capture.read(&path).is_err(), "{name}");
    }
}

#[test]
fn context_selection_resolves_links_and_parents_before_checking_containment() {
    use conglobate::path::ContextPath;
    let repo = Repository::new("sha1");
    repo.write("dir/plan", b"selected");
    repo.write("other/plan", b"other");
    repo.write("file", b"not a directory");
    for (name, target) in [
        ("selected", "dir"),
        ("escape", "../outside"),
        ("absolute", "/tmp"),
        ("loop", "loop"),
        ("broken", "missing/../dir"),
    ] {
        std::os::unix::fs::symlink(target, repo.0.path().join(name)).unwrap();
    }
    let commit = repo.commit();
    // Resolution must use committed links, even when the worktree disagrees.
    std::fs::remove_file(repo.0.path().join("selected")).unwrap();
    std::os::unix::fs::symlink("../outside", repo.0.path().join("selected")).unwrap();
    for context in ["dir", "selected", "other/../dir", "selected/../dir"] {
        let mut captured = repo.capture(&commit, context).unwrap();
        let plan = ContextPath::try_from(Path::new("plan")).unwrap();
        assert_eq!(
            captured.read(&plan).unwrap().bytes(),
            b"selected",
            "{context}"
        );
    }
    for context in [
        "escape",
        "absolute",
        "loop",
        "broken",
        "escape/../dir",
        "absolute/../dir",
        "missing/../dir",
        "file/../dir",
    ] {
        assert!(repo.capture(&commit, context).is_err(), "{context}");
    }
}

#[test]
fn input_files_preserve_symlink_and_parent_resolution() {
    use conglobate::path::ContextPath;
    let repo = Repository::new("sha1");
    repo.write("context/dir/plan", b"plan");
    repo.write("context/dir/lock", b"lock");
    for (name, target) in [
        ("escape", "../../outside"),
        ("absolute", "/tmp"),
        ("selected", "dir"),
    ] {
        std::os::unix::fs::symlink(target, repo.0.path().join("context").join(name)).unwrap();
    }
    let commit = repo.commit();
    let mut captured = repo.capture(&commit, "context").unwrap();
    for file in ["plan", "lock"] {
        for prefix in ["dir", "selected", "dir/../dir", "selected/../dir"] {
            let path = ContextPath::try_from(Path::new(&format!("{prefix}/{file}"))).unwrap();
            assert_eq!(captured.read(&path).unwrap().bytes(), file.as_bytes());
        }
        for path in [
            format!("escape/{file}"),
            format!("escape/../dir/{file}"),
            format!("absolute/../dir/{file}"),
            format!("missing/../dir/{file}"),
            format!("dir/{file}/../{file}"),
            format!("dir/{file}/"),
            format!("dir/{file}/."),
        ] {
            let path = ContextPath::try_from(Path::new(&path)).unwrap();
            assert!(captured.read(&path).is_err(), "{}", path.as_ref().display());
        }
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
    let mut capture = repo.capture(&commit, ".").unwrap();
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
    let mut capture = repo.capture(&commit, ".").unwrap();
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
    use sha2::{Digest as _, Sha256};
    let manifest = serde_json::to_vec(&crate::artifact::Manifest {
        schema_version: 2,
        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
        artifact_type: crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
        config: crate::artifact::ConfigDescriptor::canonical(),
        layers: Vec::new(),
        annotations: Default::default(),
    })
    .unwrap();
    let manifest_digest = Digest::Sha256(Sha256::digest(&manifest).into());
    let index = serde_json::to_vec(&serde_json::json!({"schemaVersion":2,
        "mediaType":"application/vnd.oci.image.index.v1+json", "manifests":[{
            "mediaType":"application/vnd.oci.image.manifest.v1+json", "artifactType":crate::artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1,
            "digest":manifest_digest.to_string(),"size":manifest.len(),
            "platform":{"os":"pichi","architecture":HostArchitecture::current().unwrap().oci_name()}}]})).unwrap();
    let index_digest = Digest::Sha256(Sha256::digest(&index).into());
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
    let dtb_digest = Digest::Sha256(Sha256::digest(&dtb).into());
    std::fs::write(blobs.join(dtb_digest.hex()), &dtb).unwrap();
    let mut with_dtb: crate::artifact::Manifest =
        serde_json::from_slice(&std::fs::read(blobs.join(manifest_digest.hex())).unwrap()).unwrap();
    with_dtb.layers.push(crate::artifact::Layer::Dtb(
        crate::artifact::DtbDescriptor {
            digest: dtb_digest.to_string(),
            size: dtb.len() as u64,
        },
    ));
    let bytes = serde_json::to_vec(&with_dtb).unwrap();
    let digest = Digest::Sha256(Sha256::digest(&bytes).into());
    std::fs::write(blobs.join(digest.hex()), &bytes).unwrap();
    let mut with_dtb_index: serde_json::Value =
        serde_json::from_slice(&std::fs::read(blobs.join(index_digest.hex())).unwrap()).unwrap();
    with_dtb_index["manifests"][0]["digest"] = digest.to_string().into();
    with_dtb_index["manifests"][0]["size"] = bytes.len().into();
    let index = serde_json::to_vec(&with_dtb_index).unwrap();
    let pin = Digest::Sha256(Sha256::digest(&index).into());
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
    let mut captured = repo.capture(&commit, ".").unwrap();
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
    let mut captured = repo.capture(&commit, ".").unwrap();
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

impl Repository {
    /// Stage a store without a module checkout; name deliberately differs from path.
    fn module(&self, child: &Repository, path: &str, name: &str) {
        let store = self.0.path().join(".git/modules").join(name);
        std::fs::create_dir_all(store.parent().unwrap()).unwrap();
        self.git(&[
            "clone",
            "--bare",
            child.0.path().to_str().unwrap(),
            store.to_str().unwrap(),
        ]);
        let nested = child.0.path().join(".git/modules");
        if nested.exists() {
            std::fs::rename(nested, store.join("modules")).unwrap();
        }
        let modules = self.0.path().join(".gitmodules");
        let mut bytes = std::fs::read_to_string(&modules).unwrap_or_default();
        bytes.push_str(&format!(
            "[submodule \"{name}\"]\n\tpath = {path}\n\turl = never-fetch.invalid/repo\n"
        ));
        self.write(".gitmodules", bytes.as_bytes());
        let id = child.git(&["rev-parse", "HEAD"]);
        self.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{id},{path}"),
        ]);
    }
}

#[test]
fn used_modules_resolve_recorded_objects_and_context_subdirectories() {
    use conglobate::path::ContextPath;
    for format in ["sha1", "sha256"] {
        let child = Repository::new(format);
        child.write("dir/file", b"module bytes");
        child.commit();
        let repo = Repository::new(format);
        repo.module(&child, "vendor/lib", "different-name");
        let commit = repo.commit();
        // Neither local config nor working .gitmodules controls resolution.
        repo.write(".gitmodules", b"invalid working metadata");
        child.write("dir/file", b"later bytes");
        child.commit();
        let mut context = repo.capture(&commit, ".").unwrap();
        let path = ContextPath::try_from(Path::new("vendor/lib/dir/file")).unwrap();
        assert_eq!(context.read(&path).unwrap().bytes(), b"module bytes");
        conglobate::copy::CopySources::prepare(&mut context, &["vendor/lib".parse().unwrap()])
            .unwrap();
        let output = tempfile::tempdir().unwrap();
        let root = std::fs::File::open(output.path()).unwrap();
        let directive = conglobate::plan::Copy::fixture(
            "vendor/lib",
            "/copied",
            rustix::process::geteuid().as_raw().to_string(),
            rustix::process::getegid().as_raw().to_string(),
            None,
        );
        conglobate::copy::CopyDestination::new(&root)
            .copy(&context, &directive)
            .unwrap();
        assert_eq!(
            std::fs::read(output.path().join("copied/dir/file")).unwrap(),
            b"module bytes"
        );
        let mut context = repo.capture(&commit, "vendor/lib/dir").unwrap();
        let path = ContextPath::try_from(Path::new("file")).unwrap();
        assert_eq!(context.read(&path).unwrap().bytes(), b"module bytes");
    }
}

#[test]
fn copy_preparation_requires_only_selected_module_stores() {
    use conglobate::copy::{CopySources, SourcePattern};
    let child = Repository::new("sha1");
    child.write("file", b"module");
    child.commit();
    let repo = Repository::new("sha1");
    repo.write("kept", b"ordinary");
    repo.module(&child, "used", "used-store");
    repo.module(&child, "unused", "unused-store");
    let commit = repo.commit();
    std::fs::remove_dir_all(repo.0.path().join(".git/modules/unused-store")).unwrap();
    let prepare = |context: &mut CapturedContext, pattern: &str| {
        CopySources::prepare(context, &[pattern.parse::<SourcePattern>().unwrap()])
    };
    let mut context = repo.capture(&commit, ".").unwrap();
    prepare(&mut context, "kept").unwrap();
    prepare(&mut context, "us*/file").unwrap();
    let error = prepare(&mut context, "unused/file")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("unused") && error.contains("needed store"),
        "{error}"
    );
    for pattern in ["unused", "/", "un*"] {
        let mut context = repo.capture(&commit, ".").unwrap();
        assert!(prepare(&mut context, pattern).is_err(), "{pattern}");
    }
}

#[test]
fn nested_module_depth_is_limited_to_eight_used_gitlinks() {
    use conglobate::{copy::CopySources, path::ContextPath};
    let mut child = Repository::new("sha1");
    child.write("file", b"deep bytes");
    child.commit();
    for _ in 0..9 {
        let parent = Repository::new("sha1");
        parent.write("file", b"at this depth");
        parent.module(&child, "nested", "store");
        parent.commit();
        child = parent;
    }
    let commit = CommitIdentity::try_from(child.git(&["rev-parse", "HEAD"]).as_str()).unwrap();
    let mut context = child.capture(&commit, ".").unwrap();
    let within = format!("{}file", "nested/".repeat(8));
    let path = ContextPath::try_from(Path::new(&within)).unwrap();
    assert_eq!(context.read(&path).unwrap().bytes(), b"at this depth");
    CopySources::prepare(&mut context, &[within.parse().unwrap()]).unwrap();
    let beyond = format!("{}file", "nested/".repeat(9));
    let path = ContextPath::try_from(Path::new(&beyond)).unwrap();
    let error = context.read(&path).unwrap_err().to_string();
    assert!(error.contains("depth") && error.contains("8"), "{error}");
    assert!(CopySources::prepare(&mut context, &["/".parse().unwrap()]).is_err());
    let mut context = child.capture(&commit, &"nested/".repeat(8)).unwrap();
    assert!(
        context
            .read(&ContextPath::try_from(Path::new("file")).unwrap())
            .is_ok()
    );
    assert!(child.capture(&commit, &"nested/".repeat(9)).is_err());
}

#[test]
fn planning_scans_used_copies_and_static_documents_before_execution() {
    use conglobate::{input::CapturedBuild, request::BuildRequest};
    let child = Repository::new("sha1");
    child.write("file", b"input");
    child.write("plan", b"tasks:\n  root:\n    kind: extend\n    from: registry.test/base:1\n    exec:\n    - copy:\n        from: module\n        into: /input\nimage: root\n");
    child.write(
        "lock",
        format!(
            "version: 1\nimages:\n  registry.test/base:1: {{index: sha256:{}}}\n",
            "11".repeat(32)
        )
        .as_bytes(),
    );
    child.commit();
    let repo = Repository::new("sha1");
    repo.module(&child, "module", "store");
    let commit = repo.commit();
    let identity = hex::encode(commit.bytes());
    let request = BuildRequest::parse_from([
        "conglobate",
        "build",
        "--repository",
        repo.0.path().to_str().unwrap(),
        "--commit",
        &identity,
        "--plan",
        "module/plan",
        "--lock",
        "module/lock",
        "--cache",
        "missing-cache",
        "--output",
        "unused-output",
        "--uid-map",
        "200000:65536",
        "--gid-map",
        "300000:65536",
    ])
    .unwrap();
    CapturedBuild::try_from(&request).unwrap();
    // Keep static documents outside the missing module to isolate COPY scanning.
    repo.write("plan", &std::fs::read(child.0.path().join("plan")).unwrap());
    repo.write("lock", &std::fs::read(child.0.path().join("lock")).unwrap());
    let commit = repo.commit();
    std::fs::remove_dir_all(repo.0.path().join(".git/modules/store")).unwrap();
    let identity = hex::encode(commit.bytes());
    let request = BuildRequest::parse_from([
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
        "missing-cache",
        "--output",
        "unused-output",
    ])
    .unwrap();
    let error = CapturedBuild::try_from(&request).unwrap_err().to_string();
    assert!(
        error.contains("module") && error.contains("needed store"),
        "{error}"
    );
    repo.write("plan", b"tasks:\n  root:\n    kind: extend\n    from: registry.test/base:1\n  unused:\n    kind: extend\n    from: registry.test/base:1\n    exec:\n    - copy:\n        from: module\n        into: /input\nimage: root\n");
    let commit = repo.commit();
    let identity = hex::encode(commit.bytes());
    let request = BuildRequest::parse_from([
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
        "missing-cache",
        "--output",
        "unused-output",
    ])
    .unwrap();
    CapturedBuild::try_from(&request).unwrap();
}

#[test]
fn module_reads_reject_missing_commits_and_host_alternates() {
    use conglobate::path::ContextPath;
    let child = Repository::new("sha1");
    child.write("file", b"input");
    child.commit();
    let repo = Repository::new("sha1");
    repo.module(&child, "module", "store");
    let commit = repo.commit();
    let store = repo.0.path().join(".git/modules/store");
    std::fs::write(
        store.join("objects/info/alternates"),
        format!("{}\n", child.0.path().join(".git/objects").display()),
    )
    .unwrap();
    let path = ContextPath::try_from(Path::new("module/file")).unwrap();
    let mut context = repo.capture(&commit, ".").unwrap();
    assert!(
        context
            .read(&path)
            .unwrap_err()
            .to_string()
            .contains("alternates")
    );
    std::fs::remove_file(store.join("objects/info/alternates")).unwrap();
    let id = child.git(&["rev-parse", "HEAD"]);
    std::fs::remove_file(store.join("objects").join(&id[..2]).join(&id[2..])).unwrap();
    let mut context = repo.capture(&commit, ".").unwrap();
    let error = context.read(&path).unwrap_err().to_string();
    assert!(error.contains("submodule"), "{error}");
}

#[test]
fn inputs_inspects_only_selected_committed_tasks_without_a_lock() {
    use conglobate::request::{CliCommand, parse_cli};
    let repo = Repository::new("sha1");
    repo.write(
        "build/plan.yaml",
        br#"
tasks:
  base:
    kind: extend
    from: registry.example/base:latest
  selected:
    kind: extend
    from: base
  unused:
    kind: extend
    from: registry.example/unused:latest
image: selected
"#,
    );
    repo.commit();
    let commit = repo.git(&["rev-parse", "HEAD"]);
    repo.write("build/plan.yaml", b"invalid working tree");
    let CliCommand::Inputs(request) = parse_cli([
        "conglobate",
        "inputs",
        "--repository",
        repo.0.path().to_str().unwrap(),
        "--commit",
        &commit,
        "--context",
        "build",
        "--plan",
        "plan.yaml",
    ])
    .unwrap() else {
        panic!("inputs command")
    };
    let inputs = request.inputs().unwrap();
    assert_eq!(
        inputs.images,
        ["registry.example/base:latest".to_owned()].into()
    );
}
