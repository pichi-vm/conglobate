// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Resolve committed gitlinks using only staged stores, without initialization.
use super::{CapturedContext, ContextError, ObjectReader, Store};
use gix::{ObjectId, object::tree::EntryKind};
use std::{ffi::OsStr, os::unix::ffi::OsStrExt, path::Path};

const MAX_DEPTH: usize = 8;

impl CapturedContext {
    pub(super) fn open_module(
        &mut self,
        parent: usize,
        path: &Path,
        oid: ObjectId,
    ) -> Result<usize, ContextError> {
        let parent = &self.stores[parent];
        let depth = parent.depth + 1;
        if depth > MAX_DEPTH {
            return Err(ContextError::Invalid("submodule depth exceeds cap of 8"));
        }
        let tree = parent.reader.tree(parent.root)?;
        let decoded = tree.decode().map_err(ContextError::git)?;
        let entry = decoded
            .entries
            .iter()
            .find(|e| e.filename.as_ref() as &[u8] == b".gitmodules")
            .ok_or(ContextError::Invalid(
                "used submodule has no committed .gitmodules",
            ))?;
        if entry.mode.kind() != EntryKind::Blob {
            return Err(ContextError::Invalid(".gitmodules is not a regular file"));
        }
        let bytes = parent.reader.blob(entry.oid.to_owned())?;
        let config = gix::config::File::from_bytes_no_includes(
            &bytes,
            gix::config::file::Metadata::default(),
            Default::default(),
        )
        .map_err(ContextError::git)?;
        let mut names = config
            .sections_by_name("submodule")
            .into_iter()
            .flatten()
            .filter_map(|section| {
                let value = section.value("path")?;
                let bytes: &[u8] = value.as_ref();
                (bytes == path.as_os_str().as_bytes())
                    .then(|| section.header().subsection_name())
                    .flatten()
            });
        let name = names.next().ok_or(ContextError::Invalid(
            "used gitlink has no .gitmodules mapping",
        ))?;
        if names.next().is_some() {
            return Err(ContextError::Invalid("ambiguous submodule path mapping"));
        }
        let name = Path::new(OsStr::from_bytes(name.as_ref()));
        if name.as_os_str().is_empty()
            || name.is_absolute()
            || name
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err(ContextError::Invalid("unsafe submodule store name"));
        }
        let modules = parent.reader.0.common_dir().join("modules");
        let location = modules.join(name);
        let canonical = location.canonicalize().map_err(|e| ContextError::Module {
            path: path.to_owned(),
            reason: format!("needed store {} is unavailable: {e}", location.display()),
        })?;
        if !canonical.starts_with(modules.canonicalize()?) {
            return Err(ContextError::Invalid(
                "submodule store escapes modules directory",
            ));
        }
        drop(tree);
        let reader = ObjectReader::open(&canonical)?;
        let root = reader
            .commit_tree(oid)
            .map_err(|error| ContextError::Module {
                path: path.to_owned(),
                reason: format!("needed commit {oid} is unavailable or invalid: {error}"),
            })?;
        let index = self.stores.len();
        self.stores.push(Store {
            reader,
            root,
            depth,
        });
        Ok(index)
    }
}
