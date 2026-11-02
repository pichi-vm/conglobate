// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ImagePath, StageRef};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum Directive {
    Copy {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from: Option<StageRef>,
        path: ImagePath,
        into: ImagePath,
    },
    Exec {
        command: String,
        #[serde(default)]
        network: bool,
        #[serde(default = "default_timeout", with = "humantime_serde")]
        timeout: Duration,
    },
}

fn default_timeout() -> Duration {
    Duration::from_secs(300)
}
