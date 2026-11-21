// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

/// Informational OCI annotations used by Pichi. Unknown keys are ignored.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OciAnnotations {
    /// Producer-supplied creation time, retained verbatim as informational metadata.
    #[serde(
        rename = "org.opencontainers.image.created",
        skip_serializing_if = "Option::is_none"
    )]
    pub created: Option<String>,
    /// Compatibility hint; platform selection uses the index descriptor.
    #[serde(
        rename = "org.opencontainers.image.architecture",
        skip_serializing_if = "Option::is_none"
    )]
    pub architecture: Option<String>,
}
