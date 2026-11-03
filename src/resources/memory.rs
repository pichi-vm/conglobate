// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;

/// Guest memory in MiB, quantised to whole 2 MiB hugepages. dillo backs guest
/// RAM with 2 MiB hugepages, so memory exists only in 2 MiB chunks; this newtype
/// makes a non-conforming size *unrepresentable* — construct only via
/// [`Memory::from_mib`] (nonzero and a multiple of [`Memory::CHUNK_MIB`]), and
/// the wire form (a MiB integer) is rejected at parse if it isn't. The invariant
/// then holds everywhere without a separate validation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Memory(NonZeroU32);

impl Memory {
    /// The hugepage chunk memory is quantised to, in MiB.
    pub const CHUNK_MIB: u32 = 2;

    /// Build from a MiB size, or `None` if it is zero or not a multiple of
    /// [`Self::CHUNK_MIB`].
    #[must_use]
    pub fn from_mib(mib: u32) -> Option<Self> {
        if mib == 0 || !mib.is_multiple_of(Self::CHUNK_MIB) {
            return None;
        }
        NonZeroU32::new(mib).map(Self)
    }

    /// The size in MiB (always a positive multiple of [`Self::CHUNK_MIB`]).
    #[must_use]
    pub fn mib(self) -> u32 {
        self.0.get()
    }
}

impl Serialize for Memory {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.0.get())
    }
}

impl<'de> Deserialize<'de> for Memory {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mib = u32::deserialize(d)?;
        Self::from_mib(mib).ok_or_else(|| {
            serde::de::Error::custom(format!(
                "memory {mib} MiB must be a positive multiple of {} (2 MiB hugepages)",
                Self::CHUNK_MIB
            ))
        })
    }
}
