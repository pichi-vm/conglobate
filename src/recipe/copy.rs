// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ImagePath, StageRef};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Copy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<StageRef>,
    pub path: ImagePath,
    pub into: ImagePath,
}
