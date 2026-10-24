// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CapturedContext, ContextEntry, ContextError};
use crate::path::ContextPath;
use std::{
    collections::VecDeque,
    ffi::OsString,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

/// A read borrow pins the captured bytes for its entire lifetime.
#[derive(Debug)]
pub(crate) struct ContextFile<'a> {
    bytes: &'a [u8],
    executable: bool,
}
impl ContextFile<'_> {
    pub(crate) fn bytes(&self) -> &[u8] {
        self.bytes
    }
    pub(crate) fn executable(&self) -> bool {
        self.executable
    }
    pub(crate) fn text(&self) -> Result<&str, ContextError> {
        std::str::from_utf8(self.bytes)
            .map_err(|_| ContextError::Invalid("selected file is not UTF-8"))
    }
}
impl CapturedContext {
    /// Resolve links against captured objects, never the caller's filesystem.
    pub(crate) fn read(&self, path: &ContextPath) -> Result<ContextFile<'_>, ContextError> {
        let mut pending: VecDeque<OsString> = path.as_ref().iter().map(OsString::from).collect();
        let mut resolved = PathBuf::new();
        let mut followed = 0;
        while let Some(part) = pending.pop_front() {
            if part == "." {
                continue;
            }
            if part == ".." {
                if !resolved.pop() {
                    return Err(ContextError::Invalid("symlink escapes context"));
                }
                continue;
            }
            resolved.push(part);
            let entry = self.entries.get(&resolved).ok_or(ContextError::Invalid(
                "selected context path does not exist",
            ))?;
            match entry {
                ContextEntry::Symlink(target) => {
                    followed += 1;
                    if followed > 40 {
                        return Err(ContextError::Invalid("too many context symlinks"));
                    }
                    let requires_directory = target.ends_with(b"/") || target.ends_with(b"/.");
                    let target = Path::new(std::ffi::OsStr::from_bytes(target));
                    if target.is_absolute() {
                        return Err(ContextError::Invalid("absolute context symlink"));
                    }
                    resolved.pop();
                    // Path::iter drops trailing separators. Keep their directory
                    // requirement so captured and kernel-backed reads agree.
                    if requires_directory {
                        pending.push_front(OsString::from("."));
                    }
                    for part in target.iter().rev() {
                        pending.push_front(part.to_owned());
                    }
                }
                ContextEntry::File { .. } if !pending.is_empty() => {
                    return Err(ContextError::Invalid(
                        "context path traverses a non-directory",
                    ));
                }
                _ => {}
            }
        }
        match self.entries.get(&resolved) {
            Some(ContextEntry::File { bytes, executable }) => Ok(ContextFile {
                bytes,
                executable: *executable,
            }),
            _ => Err(ContextError::Invalid(
                "selected context path is not a regular file",
            )),
        }
    }
}
