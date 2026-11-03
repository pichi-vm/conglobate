// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::Ingress;
use serde::{Deserialize, Serialize};

/// One declared network interface (BUILD.md §7).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    /// Human-facing description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Ports the host must/should expose inbound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ingress: Option<Ingress>,

    /// Placement hint (`pci`/`mmio`); omit to let dillo choose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
}
