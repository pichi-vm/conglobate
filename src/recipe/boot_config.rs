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

impl BootConfig {
    /// Apply this overlay after the caller resolves `from` (or an empty base).
    /// Null deletes only the exact key. Arrays replace, rather than append.
    pub fn apply(
        &self,
        base: &crate::artifact::BootConfig,
    ) -> Result<crate::artifact::BootConfig, crate::artifact::BootConfigError> {
        let mut values = base.values().clone();
        for (key, value) in &self.values {
            crate::artifact::BootConfig::check_key(key)?;
            match value {
                BootConfigValue::Delete => {
                    values.remove(key);
                }
                BootConfigValue::String(value) => {
                    values.insert(key.clone(), vec![value.clone()]);
                }
                BootConfigValue::List(value) => {
                    values.insert(key.clone(), value.clone());
                }
            }
        }
        values.try_into()
    }
}
