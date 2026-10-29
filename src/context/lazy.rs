// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Verify-on-use blob loading for regular files.
use super::{CapturedContext, ContextError};
use gix::ObjectId;

impl CapturedContext {
    /// Load and identity-check a blob. LFS pointers are rejected at use.
    pub(crate) fn blob(&self, store: usize, id: ObjectId) -> Result<Vec<u8>, ContextError> {
        let bytes = self.stores[store].reader.blob(id)?;
        if bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\n")
            || bytes.starts_with(b"version https://git-lfs.github.com/spec/v1\r\n")
        {
            return Err(ContextError::Invalid("Git LFS pointers are unsupported"));
        }
        Ok(bytes)
    }
}
