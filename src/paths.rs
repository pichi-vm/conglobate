// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Resolve image paths with chroot semantics, including absolute symlinks.
//! Callers own the tree and must have stopped every recipe writer first.

use crate::Error;
use std::{
    collections::VecDeque,
    ffi::OsString,
    path::{Component, Path, PathBuf},
};

pub fn resolve(root: &Path, path: &Path, follow_final: bool) -> Result<PathBuf, Error> {
    let mut pending = parts(path);
    let mut relative = PathBuf::new();
    let mut links = 0;
    while let Some(part) = pending.pop_front() {
        if part == "/" {
            relative.clear();
            continue;
        }
        if part == "." {
            continue;
        }
        if part == ".." {
            relative.pop();
            continue;
        }
        let candidate = root.join(&relative).join(&part);
        if follow_final || !pending.is_empty() {
            match std::fs::symlink_metadata(&candidate) {
                Ok(metadata) if metadata.is_symlink() => {
                    links += 1;
                    if links > 40 {
                        return Err(Error::Invalid("too many symlinks in image path".into()));
                    }
                    let mut target = parts(&std::fs::read_link(candidate)?);
                    target.append(&mut pending);
                    pending = target;
                    continue;
                }
                Ok(metadata) if !pending.is_empty() && !metadata.is_dir() => {
                    return Err(std::io::Error::from(std::io::ErrorKind::NotADirectory).into());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        relative.push(part);
    }
    Ok(root.join(relative))
}

fn parts(path: &Path) -> VecDeque<OsString> {
    let mut parts: VecDeque<_> = path
        .components()
        .map(|component| match component {
            Component::RootDir => OsString::from("/"),
            Component::CurDir => OsString::from("."),
            Component::ParentDir => OsString::from(".."),
            Component::Normal(name) => name.to_owned(),
            Component::Prefix(prefix) => prefix.as_os_str().to_owned(),
        })
        .collect();
    // Path::components removes a trailing slash or dot. Both require the
    // preceding component to be a directory during kernel path traversal,
    // including when they occur in a symlink's target.
    let bytes = path.as_os_str().as_encoded_bytes();
    if bytes.ends_with(b"/") || bytes.ends_with(b"/.") {
        parts.push_back(OsString::from("."));
    }
    parts
}
