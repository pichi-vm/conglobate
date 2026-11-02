// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Directive, ImagePath, ImageRef, Mount};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extend {
    pub extending: ImageRef,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub mount: BTreeMap<ImagePath, Mount>,
    pub derive: Vec<Directive>,
}
