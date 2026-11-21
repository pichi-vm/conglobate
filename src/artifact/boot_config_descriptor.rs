// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// Zstd-compressed native Linux boot configuration accompanying a PMI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BootConfigDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: super::MediaType<Self>,
    /// Content digest of the compressed blob.
    pub digest: super::Digest,
    /// Compressed blob size in bytes, independent of the PMI reservation.
    pub size: u64,
}

impl crate::artifact::Media for BootConfigDescriptor {
    const MEDIA_TYPE: &'static str = "application/vnd.pichi.bootconfig.v1+zstd";
}
