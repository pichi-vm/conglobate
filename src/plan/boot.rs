// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{
    Extract, ImageRef,
    boot_config::{Key, Value},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Boot configuration always belongs to the selected immutable PMI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Boot {
    Shorthand(ImageRef),
    Inherit {
        inherit: ImageRef,
    },
    InheritDefined {
        inherit: ImageRef,
        define: BTreeMap<Key, Value>,
    },
    InheritExtended {
        inherit: ImageRef,
        extend: BTreeMap<Key, Option<Value>>,
    },
    Extract {
        extract: Extract,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        define: BTreeMap<Key, Value>,
    },
}
