// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{AsString, Digest, ScuteVerity};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

/// Per-scute content identity and verity reconstruction annotations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScuteAnnotations {
    /// SHA-256 of the exact decompressed bytes.
    #[serde(rename = "dev.pichi.scute.digest")]
    pub digest: Digest,
    /// Exact decompressed byte count, before block padding.
    #[serde(rename = "dev.pichi.scute.size")]
    pub size: AsString<NonZeroU64>,
    /// Required verity annotations share the same flat OCI annotation map.
    #[serde(flatten)]
    pub verity: ScuteVerity,
}
