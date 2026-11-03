// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::PortSpec;
use serde::{Deserialize, Serialize};

/// Inbound port bands for an [`super::Interface`] (BUILD.md §7).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Ingress {
    /// Ports the host MUST expose, else launch errors.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required: Vec<PortSpec>,

    /// Ports the host SHOULD expose, else a warning.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommended: Vec<PortSpec>,
}
