// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{BootConfigValue, ImageRef};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// Omitting from starts a new configuration; values overlay the selected base.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BootConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<ImageRef>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, BootConfigValue>,
}
