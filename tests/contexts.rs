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
fn network_recipe_requires_an_explicit_resolver_argument() {
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
    ];
    let request = BuildRequest::parse_from(&args).unwrap();
    assert!(matches!(
        CapturedBuild::try_from(&request),
        Err(InputError::ResolverRequired)
    ));
    args.extend(["--resolv-conf", "/caller-supplied-resolver"]);
    let request = BuildRequest::parse_from(&args).unwrap();
    // This phase checks selection only; resolver capture belongs to setup.
    assert_eq!(
        CapturedBuild::try_from(&request)
            .unwrap()
            .plan()
            .network_steps()
            .len(),
        1
    );
}
