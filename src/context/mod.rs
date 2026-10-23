// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Immutable committed objects, captured without checkout filters or hooks.
mod read;
pub use read::ContextFile;

use crate::{path::ContextDirectory, request::CommitIdentity};
use gix::{ObjectId, object::tree::EntryKind};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("context I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Git input: {0}")]
    Git(String),
    #[error("invalid committed context: {0}")]
    Invalid(&'static str),
}
impl ContextError {
    fn git(error: impl std::fmt::Display) -> Self {
        Self::Git(error.to_string())
    }
}

/// Repository authority is used only during capture, never retained by the result.
#[derive(Debug)]
struct ObjectReader(gix::Repository);
impl ObjectReader {
    fn open(path: &Path) -> Result<Self, ContextError> {
        let repository = gix::open::Options::isolated()
            .open(path)
            .map_err(ContextError::git)?
            .to_thread_local();
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
}

#[derive(Debug)]
pub enum ContextEntry {
    Directory,
    File { executable: bool, bytes: Vec<u8> },
    Symlink(Vec<u8>),
}

#[derive(Debug)]
pub struct CapturedContext {
    commit: ObjectId,
    subtree: ObjectId,
    entries: BTreeMap<PathBuf, ContextEntry>,
}
impl CapturedContext {
    pub fn capture(
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
        let commit_object = reader
            .verified(commit_id)?
            .try_into_commit()
            .map_err(ContextError::git)?;
        let mut subtree = commit_object.tree_id().map_err(ContextError::git)?.detach();
        for name in context.as_ref().iter() {
            let tree = reader.tree(subtree)?;
            let decoded = tree.decode().map_err(ContextError::git)?;
            let entry = decoded
                .entries
                .iter()
                .find(|entry| {
                    let bytes: &[u8] = entry.filename.as_ref();
                    bytes == name.as_bytes()
                })
                .ok_or(ContextError::Invalid("context subtree does not exist"))?;
            if entry.mode.kind() != EntryKind::Tree {
                return Err(ContextError::Invalid(
                    "context selection is not a directory",
                ));
            }
            subtree = entry.oid.to_owned();
        }
        let mut entries = BTreeMap::new();
        let mut pending = vec![(PathBuf::new(), subtree)];
        while let Some((parent, id)) = pending.pop() {
            let tree = reader.tree(id)?;
            for entry in tree.decode().map_err(ContextError::git)?.entries {
                let name = EntryName::try_from(entry.filename.as_ref() as &[u8])?;
                let path = parent.join(name.0);
                let value = match entry.mode.kind() {
                    EntryKind::Tree => {
                        pending.push((path.clone(), entry.oid.to_owned()));
                        ContextEntry::Directory
                    }
                    EntryKind::Commit => {
                        return Err(ContextError::Invalid("submodules are unsupported"));
                    }
                    kind => {
                        let mut object = reader
                            .verified(entry.oid.to_owned())?
                            .try_into_blob()
                            .map_err(ContextError::git)?;
                        let bytes = std::mem::take(&mut object.data);
                        if bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\n")
                            || bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\r\n")
                        {
                            return Err(ContextError::Invalid("Git LFS pointers are unsupported"));
                        }
                        if kind == EntryKind::Link {
                            if bytes.is_empty() || bytes.contains(&0) {
                                return Err(ContextError::Invalid("invalid symlink contents"));
                            }
                            ContextEntry::Symlink(bytes)
                        } else {
                            ContextEntry::File {
                                executable: kind == EntryKind::BlobExecutable,
                                bytes,
                            }
                        }
                    }
                };
                if entries.insert(path, value).is_some() {
                    return Err(ContextError::Invalid("duplicate tree entry"));
                }
            }
        }
        Ok(Self {
            commit: commit_id,
            subtree,
            entries,
        })
    }
    pub fn commit(&self) -> ObjectId {
        self.commit
    }
    pub fn subtree(&self) -> ObjectId {
        self.subtree
    }
    pub fn object_format(&self) -> gix::hash::Kind {
        self.commit.kind()
    }
    pub fn entries(&self) -> &BTreeMap<PathBuf, ContextEntry> {
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
