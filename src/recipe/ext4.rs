// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{FilesystemLabel, FilesystemSize};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ext4 {
    pub size: FilesystemSize,
    #[serde(default)]
    pub label: FilesystemLabel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub esp: Option<FilesystemSize>,
}
