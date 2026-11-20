// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Informational OCI annotations used by Pichi. Unknown keys are ignored.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OciAnnotations {
    /// Image creation time, when supplied by the producer.
    #[serde(
        rename = "org.opencontainers.image.created",
        skip_serializing_if = "Option::is_none"
    )]
    pub created: Option<humantime_serde::Serde<SystemTime>>,
    /// Source repository location; does not grant access to the repository.
    #[serde(
        rename = "org.opencontainers.image.source",
        skip_serializing_if = "Option::is_none"
    )]
    pub source: Option<String>,
    /// Compatibility hint; platform selection uses the index descriptor.
    #[serde(
        rename = "org.opencontainers.image.architecture",
        skip_serializing_if = "Option::is_none"
    )]
    pub architecture: Option<String>,
}
