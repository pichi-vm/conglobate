// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Band, Interface, Memory};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, num::NonZeroU32};

/// The host-facing launch contract: what the host must provide to
/// launch a pichi artifact. Models the fields pichi acts on — `cpus`/`memory`
/// (VM sizing) and `interfaces` (network); `carapaces`/`volumes` (enforced
/// in-guest) are defined with the build effort, not here.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    /// vCPU band. The floor is 1 (a VM always has at least one vCPU), so a
    /// value of `1` carries no information and is canonicalised away; omit the
    /// band entirely to leave sizing to the operator / dillo default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<Band<NonZeroU32>>,

    /// Guest-memory band. Each tier is a [`Memory`] — MiB quantised to 2 MiB
    /// hugepages, nonzero — so a non-conforming size is unrepresentable rather
    /// than caught by a check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<Band<Memory>>,

    /// Network interfaces the host must wire (BUILD.md §7). One user-mode
    /// virtio-net device per entry; the map key is the operator-facing label.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub interfaces: BTreeMap<String, Interface>,
}
impl Requirements {
    /// Canonicalise for content-addressed emission: drop values that carry no
    /// information so semantically-equal contracts serialise identically.
    /// Currently: `cpus` has a hard floor of 1, so a `required`/`recommended`
    /// of `1` means "the floor" — indistinguishable from "unset" — and is
    /// removed; an emptied `cpus` band is dropped entirely.
    pub fn canonicalize(&mut self) {
        if let Some(b) = self.cpus.as_mut() {
            let floor = NonZeroU32::MIN; // 1
            if b.required == Some(floor) {
                b.required = None;
            }
            if b.recommended == Some(floor) {
                b.recommended = None;
            }
            if b.is_empty() {
                self.cpus = None;
            }
        }
    }

    /// Required vCPU floor, if declared (absent ⇒ the implicit floor of 1).
    #[must_use]
    pub fn cpus_required(&self) -> Option<u32> {
        self.cpus.and_then(|b| b.required).map(NonZeroU32::get)
    }

    /// Recommended vCPU count, if declared.
    #[must_use]
    pub fn cpus_recommended(&self) -> Option<u32> {
        self.cpus.and_then(|b| b.recommended).map(NonZeroU32::get)
    }

    /// Required memory floor in MiB, if declared.
    #[must_use]
    pub fn memory_required_mib(&self) -> Option<u32> {
        self.memory.and_then(|b| b.required).map(Memory::mib)
    }

    /// Recommended memory in MiB, if declared.
    #[must_use]
    pub fn memory_recommended_mib(&self) -> Option<u32> {
        self.memory.and_then(|b| b.recommended).map(Memory::mib)
    }
}
