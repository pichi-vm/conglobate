// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Components, Error, ImageRef};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Image {
    Inherit(ImageRef),
    Components(Components),
}
impl Image {
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Inherit(_) => Ok(()),
            Self::Components(components) => components.validate(),
        }
    }
    pub fn sources(&self) -> Vec<&ImageRef> {
        match self {
            Self::Inherit(source) => vec![source],
            Self::Components(components) => components
                .root
                .iter()
                .chain(components.boot.iter().map(|boot| boot.source()))
                .chain(
                    components
                        .bootconfig
                        .iter()
                        .filter_map(|config| config.source()),
                )
                .collect(),
        }
    }
}
