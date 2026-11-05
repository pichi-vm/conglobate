// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Immutable committed objects with verify-on-use blob reads.
//! Tree structure and symlink targets are resolved at capture; regular file
//! bytes are loaded and identity-checked only when read or copied.
mod lazy;
mod modules;
mod read;

use crate::{path::ContextDirectory, request::CommitIdentity};
use gix::{ObjectId, object::tree::EntryKind};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    ffi::{OsStr, OsString},
    fmt,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum ContextError {
    #[error("context I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Git input: {0}")]
    Git(String),
    #[error("invalid committed context: {0}")]
    Invalid(&'static str),
    #[error("submodule {path:?}: {reason}")]
    Module { path: PathBuf, reason: String },
}
impl ContextError {
    fn git(error: impl std::fmt::Display) -> Self {
        Self::Git(error.to_string())
    }
}

/// Retained object-store handle for verify-on-use blob loads.
struct ObjectReader(gix::Repository);
impl fmt::Debug for ObjectReader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ObjectReader")
    }
}
impl ObjectReader {
    fn open(path: &Path) -> Result<Self, ContextError> {
        let repository = gix::open::Options::isolated()
            .open(path)
            .map_err(ContextError::git)?
            .to_thread_local();
        if !repository
            .objects
            .store_ref()
            .alternate_db_paths()
            .map_err(ContextError::git)?
            .is_empty()
        {
            return Err(ContextError::Invalid(
                "host object alternates are unsupported",
            ));
        }
        Ok(Self(repository))
    }
    fn verified(&self, id: ObjectId) -> Result<gix::Object<'_>, ContextError> {
        let object = self.0.find_object(id).map_err(ContextError::git)?;
        let actual = gix::objs::compute_hash(id.kind(), object.kind, &object.data)
            .map_err(ContextError::git)?;
        if actual != id {
            return Err(ContextError::Invalid(
                "object content does not match its identity",
            ));
        }
        Ok(object)
    }
    fn tree(&self, id: ObjectId) -> Result<gix::Tree<'_>, ContextError> {
        let tree = self
            .verified(id)?
            .try_into_tree()
            .map_err(ContextError::git)?;
        let decoded = tree.decode().map_err(ContextError::git)?;
        let mut names = BTreeSet::new();
        for entry in &decoded.entries {
            let bytes: &[u8] = entry.filename.as_ref();
            EntryName::try_from(bytes)?;
            if !names.insert(bytes) {
                return Err(ContextError::Invalid("duplicate tree entry"));
            }
        }
        Ok(tree)
    }
    pub(super) fn blob(&self, id: ObjectId) -> Result<Vec<u8>, ContextError> {
        let mut object = self
            .verified(id)?
            .try_into_blob()
            .map_err(ContextError::git)?;
        Ok(std::mem::take(&mut object.data))
    }
}

#[derive(Debug)]
pub(crate) enum ContextEntry {
    Directory,
    Module {
        store: usize,
        path: PathBuf,
        oid: ObjectId,
    },
    /// Blob oid only; bytes are loaded through the retained reader on use.
    File {
        store: usize,
        executable: bool,
        oid: ObjectId,
    },
    Symlink(Vec<u8>),
}

#[derive(Debug)]
pub(crate) struct CapturedContext {
    commit: ObjectId,
    subtree: ObjectId,
    stores: Vec<Store>,
    entries: BTreeMap<PathBuf, ContextEntry>,
}
impl CapturedContext {
    pub(crate) fn capture(
        repository: &Path,
        commit: &CommitIdentity,
        context: &ContextDirectory,
    ) -> Result<Self, ContextError> {
        let reader = ObjectReader::open(repository)?;
        let commit_id = ObjectId::from_hex(hex::encode(commit.bytes()).as_bytes())
            .map_err(ContextError::git)?;
        if commit_id.kind() != reader.0.object_hash() {
            return Err(ContextError::Invalid(
                "commit object format differs from repository",
            ));
        }
        let root = reader.commit_tree(commit_id)?;
        let mut captured = Self {
            commit: commit_id,
            subtree: root,
            stores: vec![Store {
                reader,
                root,
                depth: 0,
            }],
            entries: BTreeMap::new(),
        };
        let mut store = 0;
        let mut subtree = root;
        let mut repository_path = PathBuf::new();
        let mut parents = Vec::new();
        let mut pending: VecDeque<OsString> = context.as_ref().iter().map(OsString::from).collect();
        let mut followed = 0;
        while let Some(name) = pending.pop_front() {
            if name == "." {
                continue;
            }
            if name == ".." {
                (store, subtree, repository_path) = parents.pop().ok_or(ContextError::Invalid(
                    "context selection escapes repository",
                ))?;
                continue;
            }
            let tree = captured.stores[store].reader.tree(subtree)?;
            let decoded = tree.decode().map_err(ContextError::git)?;
            let entry = decoded
                .entries
                .iter()
                .find(|entry| {
                    let bytes: &[u8] = entry.filename.as_ref();
                    bytes == name.as_bytes()
                })
                .ok_or(ContextError::Invalid("context subtree does not exist"))?;
            let id = entry.oid.to_owned();
            let kind = entry.mode.kind();
            drop(tree);
            match kind {
                EntryKind::Tree => {
                    parents.push((store, subtree, repository_path.clone()));
                    repository_path.push(name);
                    subtree = id;
                }
                EntryKind::Commit => {
                    parents.push((store, subtree, repository_path.clone()));
                    repository_path.push(name);
                    store = captured.open_module(store, &repository_path, id)?;
                    subtree = captured.stores[store].root;
                    repository_path.clear();
                }
                EntryKind::Link => {
                    followed += 1;
                    if followed > 40 {
                        return Err(ContextError::Invalid("too many context symlinks"));
                    }
                    let target = captured.stores[store].reader.blob(id)?;
                    if target.is_empty() || target.contains(&0) {
                        return Err(ContextError::Invalid("invalid symlink contents"));
                    }
                    let target = Path::new(OsStr::from_bytes(&target));
                    if target.is_absolute() {
                        return Err(ContextError::Invalid("absolute context symlink"));
                    }
                    for part in target.iter().rev() {
                        pending.push_front(part.to_owned());
                    }
                }
                _ => {
                    return Err(ContextError::Invalid(
                        "context selection is not a directory",
                    ));
                }
            }
        }
        captured.subtree = subtree;
        captured.scan(store, PathBuf::new(), repository_path, subtree)?;
        Ok(captured)
    }

    fn scan(
        &mut self,
        store: usize,
        prefix: PathBuf,
        repository_path: PathBuf,
        id: ObjectId,
    ) -> Result<(), ContextError> {
        let reader = &self.stores[store].reader;
        let mut pending = vec![(prefix, repository_path, id)];
        while let Some((parent, repository_path, id)) = pending.pop() {
            let tree = reader.tree(id)?;
            for entry in tree.decode().map_err(ContextError::git)?.entries {
                let name = EntryName::try_from(entry.filename.as_ref() as &[u8])?;
                let path = parent.join(name.0);
                let module_path = repository_path.join(name.0);
                let oid = entry.oid.to_owned();
                let value = match entry.mode.kind() {
                    EntryKind::Tree => {
                        pending.push((path.clone(), module_path, oid));
                        ContextEntry::Directory
                    }
                    EntryKind::Commit => ContextEntry::Module {
                        store,
                        path: module_path,
                        oid,
                    },
                    EntryKind::Link => {
                        let bytes = reader.blob(oid)?;
                        if bytes.is_empty() || bytes.contains(&0) {
                            return Err(ContextError::Invalid("invalid symlink contents"));
                        }
                        ContextEntry::Symlink(bytes)
                    }
                    kind => ContextEntry::File {
                        store,
                        executable: kind == EntryKind::BlobExecutable,
                        oid,
                    },
                };
                if self.entries.insert(path, value).is_some() {
                    return Err(ContextError::Invalid("duplicate tree entry"));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn expand(&mut self, selected: &Path) -> Result<(), ContextError> {
        let Some(ContextEntry::Module { store, path, oid }) = self.entries.get(selected) else {
            return Ok(());
        };
        let store = self
            .open_module(*store, &path.clone(), *oid)
            .map_err(|error| ContextError::Module {
                path: selected.to_owned(),
                reason: error.to_string(),
            })?;
        self.scan(
            store,
            selected.to_owned(),
            PathBuf::new(),
            self.stores[store].root,
        )?;
        self.entries
            .insert(selected.to_owned(), ContextEntry::Directory);
        Ok(())
    }
    /// Bind a selector without publishing its name or the selected tree identity.
    pub(crate) fn path_commitment(&self, key: &str, path: &Path) -> crate::artifact::Digest {
        use sha2::{Digest as _, Sha256};
        let mut hash = Sha256::new();
        hash.update(key.as_bytes());
        hash.update([0]);
        hash.update(self.subtree.as_bytes());
        hash.update([0]);
        hash.update(path.as_os_str().as_bytes());
        crate::artifact::Digest::Sha256(hash.finalize().into())
    }
    pub(crate) fn commit(&self) -> ObjectId {
        self.commit
    }
    #[cfg(test)]
    pub(crate) fn subtree(&self) -> ObjectId {
        self.subtree
    }
    pub(crate) fn entries(&self) -> &BTreeMap<PathBuf, ContextEntry> {
        &self.entries
    }
}

#[derive(Debug)]
struct EntryName<'a>(&'a OsStr);
impl<'a> TryFrom<&'a [u8]> for EntryName<'a> {
    type Error = ContextError;
    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        if bytes.is_empty()
            || bytes == b"."
            || bytes == b".."
            || bytes.eq_ignore_ascii_case(b".git")
            || bytes.contains(&b'/')
            || bytes.contains(&0)
        {
            return Err(ContextError::Invalid("invalid tree entry name"));
        }
        Ok(Self(OsStr::from_bytes(bytes)))
    }
}

#[derive(Debug)]
struct Store {
    reader: ObjectReader,
    root: ObjectId,
    depth: usize,
}

impl ObjectReader {
    fn commit_tree(&self, id: ObjectId) -> Result<ObjectId, ContextError> {
        if id.kind() != self.0.object_hash() {
            return Err(ContextError::Invalid(
                "module object format differs from repository",
            ));
        }
        Ok(self
            .verified(id)?
            .try_into_commit()
            .map_err(ContextError::git)?
            .tree_id()
            .map_err(ContextError::git)?
            .detach())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
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
        ) -> Result<CapturedContext, crate::context::ContextError> {
            CapturedContext::capture(
                self.0.path(),
                commit,
                &ContextDirectory::try_from(Path::new(context)).unwrap(),
            )
        }
    }

    #[test]
    fn capture_uses_verified_commit_and_subtree_in_both_object_formats() {
        use crate::path::ContextPath;
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
        use crate::path::ContextPath;
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
        use crate::path::ContextPath;
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
        use crate::path::ContextPath;
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
        use crate::path::ContextPath;
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
        use crate::path::ContextPath;
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
        use crate::path::ContextPath;
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
        let path = crate::path::ContextPath::try_from(Path::new("plan")).unwrap();
        assert_eq!(capture.read(&path).unwrap().bytes(), b"original\n");
    }

    #[test]
    fn captured_bytes_modes_and_links_survive_source_changes() {
        use crate::path::ContextPath;
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
        use crate::path::ContextPath;
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
    #[ignore = "requires root to assign image ownership"]
    fn used_modules_resolve_recorded_objects_and_context_subdirectories() {
        use crate::path::ContextPath;
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
            crate::copy::CopySources::prepare(&mut context, &["vendor/lib".parse().unwrap()])
                .unwrap();
            let output = tempfile::tempdir().unwrap();
            let root = std::fs::File::open(output.path()).unwrap();
            let directive = crate::planner::Copy::fixture("vendor/lib", "/copied");
            crate::copy::CopyDestination::new(&root)
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
        use crate::copy::{CopySources, SourcePattern};
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
        use crate::{copy::CopySources, path::ContextPath};
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
    fn module_reads_reject_missing_commits_and_host_alternates() {
        use crate::path::ContextPath;
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
}
