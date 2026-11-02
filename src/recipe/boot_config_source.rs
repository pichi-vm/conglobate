// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{BootConfig, ImageRef};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BootConfigSource {
    Inherit(ImageRef),
    Overlay(BootConfig),
}
impl BootConfigSource {
    pub fn source(&self) -> Option<&ImageRef> {
        match self {
            Self::Inherit(source) => Some(source),
            Self::Overlay(config) => config.from.as_ref(),
        }
    }
}
