// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Boot, BootConfigSource, Error, ImageRef};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Components {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<ImageRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boot: Option<Boot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootconfig: Option<BootConfigSource>,
}
impl Components {
    pub fn validate(&self) -> Result<(), Error> {
        if self.bootconfig.is_some() && self.boot.is_none() {
            return Err(Error::from(
                "bootconfig requires an explicitly selected PMI",
            ));
        }
        if self.root.is_none() && self.boot.is_none() {
            return Err(Error::from("image must select a root or PMI"));
        }
        if let Some(BootConfigSource::Overlay(overlay)) = &self.bootconfig {
            overlay
                .apply(&crate::artifact::BootConfig::default())
                .map_err(|error| Error::from(error.to_string()))?;
        }
        Ok(())
    }
}
