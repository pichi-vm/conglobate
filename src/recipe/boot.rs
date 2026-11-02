// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{BootFile, ImageRef};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Boot {
    Inherit(ImageRef),
    File(BootFile),
}
impl Boot {
    pub fn source(&self) -> &ImageRef {
        match self {
            Self::Inherit(source) => source,
            Self::File(file) => &file.from,
        }
    }
}
