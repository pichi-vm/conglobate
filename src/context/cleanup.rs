// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::{self, File},
    io,
};

/// Cleanup-only authority for a private context directory. Partial deletion does
/// not restore a staged or read-only checkout. Parent identity stays pinned so
/// the proc-fd path remains valid across every retry.
#[derive(Debug, thiserror::Error)]
#[error("context backing cleanup: {source}")]
pub struct ContextCleanupFailure {
    source: io::Error,
    backing: ContextCleanup,
}
impl ContextCleanupFailure {
    pub fn source_error(&self) -> &io::Error {
        &self.source
    }
    pub fn retry(self) -> Result<(), Self> {
        self.backing.close()
    }
}

#[derive(Debug)]
pub struct ContextCleanup {
    // TempDir fallback removal must run before the proc-fd parent is closed.
    directory: tempfile::TempDir,
    _parent: File,
}
impl ContextCleanup {
    pub(super) fn new(directory: tempfile::TempDir, parent: File) -> Self {
        Self {
            directory,
            _parent: parent,
        }
    }
    pub fn close(self) -> Result<(), ContextCleanupFailure> {
        // TempDir::close consumes the owner even on failure. Borrow the path
        // during removal instead, retaining both owners on any partial failure.
        match fs::remove_dir_all(self.directory.path()) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(ContextCleanupFailure {
                    source,
                    backing: self,
                });
            }
        }
        // Removal succeeded; disarm redundant destructor traversal.
        let Self { directory, _parent } = self;
        let _ = directory.keep();
        drop(_parent);
        Ok(())
    }
}
