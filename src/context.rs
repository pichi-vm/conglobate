// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Verified committed input, independent of the caller's working tree.

use crate::{
    Error,
    recipe::{ImageLock, Pipeline, RecipeFiles},
};
use gix::{ObjectId, object::tree::EntryKind};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    fs::OpenOptions,
    io::Write,
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{Component, Path, PathBuf},
};

#[derive(Debug)]
struct Entry {
    kind: EntryKind,
    bytes: Vec<u8>,
}

/// Captured, hash-verified objects. No later read consults the source repository.
#[derive(Debug)]
pub struct Context {
    pub commit: String,
    pub tree: String,
    pub object_format: String,
    files: BTreeMap<PathBuf, Entry>,
}

fn git_error(error: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("Git input: {error}"))
}

fn object(repo: &gix::Repository, id: ObjectId) -> Result<gix::Object<'_>, Error> {
    let object = repo.find_object(id).map_err(git_error)?;
    let actual =
        gix::objs::compute_hash(id.kind(), object.kind, &object.data).map_err(git_error)?;
    if actual != id {
        return Err(git_error(format!("object hash mismatch: {id}")));
    }
    Ok(object)
}

impl Context {
    pub fn capture(repository: &Path, commit: &str, context: &Path) -> Result<Self, Error> {
        // Explicit object IDs only: no revision expressions, refs or tag peeling.
        let id = ObjectId::from_hex(commit.as_bytes()).map_err(git_error)?;
        let repo = gix::open(repository).map_err(git_error)?;
        if id.kind() != repo.object_hash() {
            return Err(git_error("commit uses the wrong object format"));
        }
        let commit_object = object(&repo, id)?.try_into_commit().map_err(git_error)?;
        let mut tree_id = commit_object.tree_id().map_err(git_error)?.detach();
        for component in context.components() {
            let Component::Normal(name) = component else {
                if component == Component::CurDir {
                    continue;
                }
                return Err(git_error("context must be a relative subtree"));
            };
            let tree = object(&repo, tree_id)?.try_into_tree().map_err(git_error)?;
            let decoded = tree.decode().map_err(git_error)?;
            let entry = decoded
                .entries
                .iter()
                .find(|entry| entry.filename.as_ref() as &[u8] == name.as_bytes())
                .ok_or_else(|| git_error("context subtree does not exist"))?;
            if entry.mode.kind() != EntryKind::Tree {
                return Err(git_error("context is not a tree"));
            }
            tree_id = entry.oid.to_owned();
        }
        let mut files = BTreeMap::new();
        let mut pending = vec![(PathBuf::new(), tree_id)];
        while let Some((directory, id)) = pending.pop() {
            let tree = object(&repo, id)?.try_into_tree().map_err(git_error)?;
            for entry in tree.decode().map_err(git_error)?.entries {
                let name: &[u8] = entry.filename.as_ref();
                if name.is_empty()
                    || name == b"."
                    || name == b".."
                    || name.eq_ignore_ascii_case(b".git")
                    || name.contains(&b'/')
                    || name.contains(&0)
                {
                    return Err(git_error("unsafe tree entry name"));
                }
                let path = directory.join(OsStr::from_bytes(name));
                let kind = entry.mode.kind();
                if files.contains_key(&path) {
                    return Err(git_error("duplicate tree entry"));
                }
                if kind == EntryKind::Commit {
                    return Err(git_error("submodules are not supported"));
                }
                let bytes = if kind == EntryKind::Tree {
                    pending.push((path.clone(), entry.oid.to_owned()));
                    Vec::new()
                } else {
                    let blob = object(&repo, entry.oid.to_owned())?
                        .try_into_blob()
                        .map_err(git_error)?;
                    if blob
                        .data
                        .starts_with(b"version https://git-lfs.github.com/spec/v1\n")
                        || blob
                            .data
                            .starts_with(b"version https://git-lfs.github.com/spec/v1\r\n")
                    {
                        return Err(git_error("Git LFS pointers are not supported"));
                    }
                    blob.data.clone()
                };
                files.insert(path, Entry { kind, bytes });
            }
        }
        Ok(Self {
            commit: id.to_string(),
            tree: tree_id.to_string(),
            object_format: match repo.object_hash() {
                gix::hash::Kind::Sha1 => "sha1",
                _ => "sha256",
            }
            .into(),
            files,
        })
    }

    pub(crate) fn recipe(&self, name: &str) -> Result<Option<&str>, Error> {
        let path = Path::new("pichi.build").join(name);
        // Resolve committed symlinks using only captured objects. Never follow
        // a recipe link through the caller's filesystem or outside the context.
        let mut pending: std::collections::VecDeque<_> = path
            .components()
            .map(|part| part.as_os_str().to_owned())
            .collect();
        let mut resolved = PathBuf::new();
        let mut links = 0;
        while let Some(part) = pending.pop_front() {
            if part == "." {
                continue;
            }
            if part == ".." {
                if !resolved.pop() {
                    return Err(git_error("recipe symlink escapes the context"));
                }
                continue;
            }
            resolved.push(part);
            let Some(entry) = self.files.get(&resolved) else {
                if links == 0 {
                    return Ok(None);
                }
                return Err(git_error("recipe symlink target does not exist"));
            };
            if entry.kind == EntryKind::Link {
                links += 1;
                if links > 40 {
                    return Err(git_error("too many recipe symlinks"));
                }
                let target = Path::new(OsStr::from_bytes(&entry.bytes));
                if target.is_absolute() || entry.bytes.contains(&0) || entry.bytes.is_empty() {
                    return Err(git_error("recipe symlink must be context-relative"));
                }
                resolved.pop();
                for part in target.components().rev() {
                    pending.push_front(part.as_os_str().to_owned());
                }
            } else if !pending.is_empty() && entry.kind != EntryKind::Tree {
                return Err(git_error("recipe parent is not a directory"));
            }
        }
        let entry = self
            .files
            .get(&resolved)
            .ok_or_else(|| git_error("recipe target is not a file"))?;
        if !matches!(entry.kind, EntryKind::Blob | EntryKind::BlobExecutable) {
            return Err(git_error(format!(
                "recipe {} must be a regular file",
                path.display()
            )));
        }
        std::str::from_utf8(&entry.bytes)
            .map(Some)
            .map_err(git_error)
    }

    pub fn pipeline(&self) -> Result<(Pipeline, ImageLock), Error> {
        let pipeline = Pipeline::try_from(RecipeFiles {
            bootstrap: self.recipe("bootstrap.yaml")?,
            carapace: self.recipe("carapace.yaml")?,
            firmware: self.recipe("firmware.yaml")?,
            platform: self.recipe("platform.yaml")?,
            transform: self.recipe("transform.yaml")?,
        })?;
        let lock = ImageLock::parse(
            self.recipe("refs.lock")?
                .ok_or_else(|| git_error("missing refs.lock"))?,
        )?;
        lock.validate(&pipeline)?;
        Ok((pipeline, lock))
    }

    /// Materialize into a new private directory on the caller's tmpfs. The
    /// executor must bind-mount it read-only before exposing it to a build step.
    pub fn materialize(&self, destination: &Path) -> Result<(), Error> {
        std::fs::create_dir(destination)?;
        for (path, entry) in &self.files {
            let target = destination.join(path);
            match entry.kind {
                EntryKind::Tree => std::fs::create_dir(target)?,
                EntryKind::Link => {
                    std::os::unix::fs::symlink(OsStr::from_bytes(&entry.bytes), target)?;
                }
                EntryKind::Blob | EntryKind::BlobExecutable => {
                    OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .mode(if entry.kind == EntryKind::BlobExecutable {
                            0o777
                        } else {
                            0o666
                        })
                        .open(target)?
                        .write_all(&entry.bytes)?;
                }
                EntryKind::Commit => unreachable!("submodules rejected at capture"),
            }
        }
        Ok(())
    }
}
