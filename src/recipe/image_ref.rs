// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Remote, StageRef};
use serde::{Deserialize, Serialize};

/// Disjoint local-stage and qualified-registry namespaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImageRef {
    Local(StageRef),
    Remote(Remote),
}
