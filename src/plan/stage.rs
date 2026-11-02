// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Directive, Format, ImagePath, ImageRef, Mount};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Stage {
    Create {
        r#use: ImageRef,
        #[serde(default)]
        format: Format,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        mounts: BTreeMap<ImagePath, Mount>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        steps: Vec<Directive>,
    },
    Extend {
        extend: ImageRef,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        mounts: BTreeMap<ImagePath, Mount>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        steps: Vec<Directive>,
    },
}
