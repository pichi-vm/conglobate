// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Immutable committed objects with verify-on-use blob reads.
//! Tree structure and symlink targets are resolved at capture; regular file
//! bytes are loaded and identity-checked only when read or copied.
mod lazy;
mod modules;
mod read;
pub(crate) use read::ContextFile;

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
    #[cfg(test)]
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
            #[cfg(test)]
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
        #[cfg(test)]
        {
            captured.subtree = subtree;
        }
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
