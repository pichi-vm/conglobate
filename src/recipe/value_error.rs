// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValueError {
    #[error("expected a positive decimal integer followed by an explicit unit")]
    Syntax,
    #[error("value overflows its supported representation")]
    Overflow,
    #[error(
        "filesystem size must be 4096-byte aligned, at least 16 MiB, and representable as a byte count"
    )]
    Capacity,
    #[error("ext4 label must be at most 16 UTF-8 bytes and contain no NUL")]
    Label,
}
