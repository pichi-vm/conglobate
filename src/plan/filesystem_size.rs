// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::num::NonZeroU32;

/// Positive filesystem capacity stored in whole MiB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemSize(NonZeroU32);

impl FilesystemSize {
    pub fn new(mib: u32) -> Option<Self> {
        NonZeroU32::new(mib).map(Self)
    }
    pub fn bytes(self) -> u64 {
        u64::from(self.0.get()) << 20
    }
    pub fn blocks(self) -> u64 {
        self.bytes() / 4096
    }
}
impl Default for FilesystemSize {
    fn default() -> Self {
        Self::new(256 * 1024).unwrap()
    }
}
impl Serialize for FilesystemSize {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mib = self.0.get();
        if mib.is_multiple_of(1024 * 1024) {
            serializer.collect_str(&format_args!("{}TiB", mib / (1024 * 1024)))
        } else if mib.is_multiple_of(1024) {
            serializer.collect_str(&format_args!("{}GiB", mib / 1024))
        } else {
            serializer.collect_str(&format_args!("{mib}MiB"))
        }
    }
}
impl<'de> Deserialize<'de> for FilesystemSize {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        for (unit, factor) in [("MiB", 1), ("GiB", 1024), ("TiB", 1024 * 1024)] {
            if let Some(number) = text.strip_suffix(unit)
                && !number.is_empty()
                && number.bytes().all(|b| b.is_ascii_digit())
                && let Some(size) = number
                    .parse::<u32>()
                    .ok()
                    .and_then(|n| n.checked_mul(factor))
                    .and_then(Self::new)
            {
                return Ok(size);
            }
        }
        Err(serde::de::Error::custom(
            "expected a positive whole MiB, GiB or TiB capacity fitting u32 MiB",
        ))
    }
}
