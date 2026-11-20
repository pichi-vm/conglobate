// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// A salt must contain its parent-root prefix and fit the dm-verity header.
#[derive(Debug, thiserror::Error)]
pub enum SaltError {
    /// The annotation is not hex.
    #[error("invalid salt: {0}")]
    Hex(#[from] hex::FromHexError),
    /// The header supports at most 256 bytes, including the parent prefix.
    #[error("salt must contain 32 to 256 bytes, got {0}")]
    Length(usize),
}
