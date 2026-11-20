// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{AsString, Digest, Salt};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;

/// Per-scute verity inputs and the identity of the complete generated file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScuteVerity {
    /// COW length rounded up to chain-wide data blocks.
    #[serde(rename = "dev.pichi.scute.verity.data-blocks")]
    pub data_blocks: AsString<NonZeroU64>,
    /// Salt containing the parent-root prefix. Consumers verify that prefix
    /// against the previous scute; the manifest checks base-to-tip ordering.
    #[serde(rename = "dev.pichi.scute.verity.salt")]
    pub salt: Salt,
    /// SHA-256 of the complete verity file, including header and padding.
    #[serde(rename = "dev.pichi.scute.verity.digest")]
    pub digest: Digest,
}
