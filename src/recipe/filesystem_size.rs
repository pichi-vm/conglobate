// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ValueError, quantity::Quantity};
use std::{num::NonZeroU64, str::FromStr};
/// Filesystem capacity; the carapace logical device geometry is independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FilesystemSize(NonZeroU64);

impl FilesystemSize {
    pub fn bytes(self) -> u64 {
        self.0.get()
    }

    pub fn blocks(self) -> u64 {
        self.bytes() / 4096
    }
}

impl FromStr for FilesystemSize {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let quantity = Quantity::try_from(text)?;
        let factor = match quantity.unit {
            "B" => 1,
            "KiB" => 1024,
            "MiB" => 1024 * 1024,
            "GiB" => 1024 * 1024 * 1024,
            _ => return Err(ValueError::Syntax),
        };
        let bytes = quantity
            .integer
            .get()
            .checked_mul(factor)
            .ok_or(ValueError::Overflow)?;
        Self::try_from(bytes)
    }
}

impl TryFrom<u64> for FilesystemSize {
    type Error = ValueError;

    fn try_from(bytes: u64) -> Result<Self, Self::Error> {
        if bytes < 16 * 1024 * 1024 || !bytes.is_multiple_of(4096) {
            return Err(ValueError::Capacity);
        }
        Ok(Self(NonZeroU64::new(bytes).ok_or(ValueError::Capacity)?))
    }
}

impl TryFrom<String> for FilesystemSize {
    type Error = ValueError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
impl From<FilesystemSize> for String {
    fn from(value: FilesystemSize) -> Self {
        format!("{}B", value.bytes())
    }
}
