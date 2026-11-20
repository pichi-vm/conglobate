// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{AsString, Commit};
use crate::is_default::IsDefault;
use serde::{Deserialize, Serialize};

/// Captured source inputs for a build. These are provenance, not authority to
/// access the repository or resolve paths on the consumer's filesystem.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuildProvenance {
    /// Full resolved Git commit identity.
    #[serde(rename = "dev.pichi.build.commit")]
    pub commit: AsString<Commit>,
    /// Path inside the commit; `.` denotes its root.
    #[serde(rename = "dev.pichi.build.context")]
    pub context: String,
    /// Plan path relative to the selected context.
    #[serde(rename = "dev.pichi.build.plan")]
    pub plan: String,
    /// Lock path relative to the context, if a lock document was consumed.
    #[serde(
        rename = "dev.pichi.build.lock",
        skip_serializing_if = "Option::is_none"
    )]
    pub lock: Option<String>,
    /// Repository location hint, if supplied by the caller.
    #[serde(
        rename = "dev.pichi.build.location",
        skip_serializing_if = "Option::is_none"
    )]
    pub location: Option<String>,
    /// Whether this build selected a bootstrap tool.
    #[serde(
        rename = "dev.pichi.build.bootstrapped",
        default,
        skip_serializing_if = "IsDefault::is_default"
    )]
    pub bootstrapped: AsString<bool>,
}
