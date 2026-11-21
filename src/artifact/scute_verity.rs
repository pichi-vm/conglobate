// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{AsString, Digest};
use devmap_verity::Salt;
use serde::{Deserialize, Serialize};

/// Per-scute verity file identity and reconstruction inputs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScuteVerity {
    /// Full salt as hex, including the parent-root prefix. For non-base scutes,
    /// that prefix must equal the previous scute's verity root.
    /// Manifest validation checks prefix length and base/non-base ordering;
    /// cache chain validation checks prepared header links, not equality
    /// between annotation and header salts.
    #[serde(rename = "dev.pichi.scute.verity.salt")]
    pub salt: AsString<Salt>,
    /// Required SHA-256 of the complete generated verity file, including its
    /// header and padding. This is distinct from the verity root.
    #[serde(rename = "dev.pichi.scute.verity.digest")]
    pub digest: Digest,
}
