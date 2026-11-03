// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

/// A two-tier requirement (BUILD.md §7): `required` MUST be met or launch
/// errors; `recommended` SHOULD be met or the instance starts with a warning.
/// Neither is a ceiling; both are individually optional.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, bound(serialize = "T: Serialize"))]
pub struct Band<T> {
    // Private: the ordering invariant (`recommended ≥ required`) is enforced by
    // `new`/`Deserialize`, so bands can only be built through a checked path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) required: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) recommended: Option<T>,
}

// Manual `Default` (the derive would wrongly require `T: Default`; an empty band
// is `None`/`None` regardless of `T`, so `NonZeroU32`/`Memory` bands work).
impl<T> Default for Band<T> {
    fn default() -> Self {
        Self {
            required: None,
            recommended: None,
        }
    }
}

impl<T: PartialOrd> Band<T> {
    /// Build a band, or `None` if `recommended` is below `required` — the sole
    /// *relational* invariant (it relates two values, so it can't be a value
    /// newtype). This is the one place it's checked; [`Deserialize`] routes
    /// through it, so any band you hold — from code or the wire — is ordered.
    #[must_use]
    pub fn new(required: Option<T>, recommended: Option<T>) -> Option<Self> {
        if let (Some(req), Some(rec)) = (&required, &recommended)
            && rec < req
        {
            return None;
        }
        Some(Self {
            required,
            recommended,
        })
    }
}

impl<T: Copy> Band<T> {
    /// The `required` tier, if set.
    #[must_use]
    pub fn required(self) -> Option<T> {
        self.required
    }

    /// The `recommended` tier, if set.
    #[must_use]
    pub fn recommended(self) -> Option<T> {
        self.recommended
    }
}

impl<T> Band<T> {
    /// True when neither tier is set (nothing to say).
    pub(super) fn is_empty(&self) -> bool {
        self.required.is_none() && self.recommended.is_none()
    }
}

/// Deserialization mirror: parse the two tiers, then route through
/// [`Band::new`] so the ordering invariant holds for wire input too.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, bound(deserialize = "T: Deserialize<'de>"))]
struct BandWire<T> {
    #[serde(default)]
    pub(super) required: Option<T>,
    #[serde(default)]
    pub(super) recommended: Option<T>,
}

impl<'de, T> Deserialize<'de> for Band<T>
where
    T: Deserialize<'de> + PartialOrd,
{
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let w = BandWire::<T>::deserialize(d)?;
        Band::new(w.required, w.recommended)
            .ok_or_else(|| serde::de::Error::custom("recommended is below required"))
    }
}
