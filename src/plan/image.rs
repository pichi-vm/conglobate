// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Boot, ImageRef};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Image {
    Shorthand(ImageRef),
    Components {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        root: Option<ImageRef>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        boot: Option<Boot>,
    },
}
