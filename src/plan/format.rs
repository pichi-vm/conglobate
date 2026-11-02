// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Filesystem, FilesystemSize};
use serde::{Deserialize, Serialize};

/// Root filesystem settings; GPT and partition identities are builder policy.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Format {
    #[serde(default, rename = "fs")]
    pub filesystem: Filesystem,
    #[serde(default)]
    pub size: FilesystemSize,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esp: Option<FilesystemSize>,
}
