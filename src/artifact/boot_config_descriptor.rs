// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// Zstd-compressed native Linux boot configuration accompanying a PMI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BootConfigDescriptor {
    /// Content digest of the compressed blob.
    pub digest: String,
    /// Compressed blob size in bytes, independent of the PMI reservation.
    pub size: u64,
}
