// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Immutable validated scalar inputs. No filesystem or process authority.
use std::{num::NonZeroU64, str::FromStr, time::Duration};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValueError {
    #[error("expected a positive decimal integer followed by an explicit unit")]
    Syntax,
    #[error("value overflows its supported representation")]
    Overflow,
    #[error("filesystem size must be 4096-byte aligned and between 16 MiB and 64 GiB")]
    Capacity,
    #[error("ext4 label must be at most 16 UTF-8 bytes and contain no NUL")]
    Label,
}

#[derive(Debug)]
struct Quantity<'a> {
    integer: NonZeroU64,
    unit: &'a str,
}

impl<'a> TryFrom<&'a str> for Quantity<'a> {
    type Error = ValueError;

    fn try_from(text: &'a str) -> Result<Self, Self::Error> {
        let boundary = text
            .find(|c: char| !c.is_ascii_digit())
            .ok_or(ValueError::Syntax)?;
        if boundary == 0 {
            return Err(ValueError::Syntax);
        }
        let integer = text[..boundary]
            .parse::<u64>()
            .map_err(|_| ValueError::Overflow)?;
        Ok(Self {
            integer: NonZeroU64::new(integer).ok_or(ValueError::Syntax)?,
            unit: &text[boundary..],
        })
    }
}

/// Filesystem capacity; the carapace logical device geometry is independent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemSize(NonZeroU64);

impl FilesystemSize {
    pub const LOGICAL_DEVICE_BYTES: u64 = 64 * 1024 * 1024 * 1024;

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
        if !(16 * 1024 * 1024..=Self::LOGICAL_DEVICE_BYTES).contains(&bytes) || bytes % 4096 != 0 {
            return Err(ValueError::Capacity);
        }
        Ok(Self(NonZeroU64::new(bytes).ok_or(ValueError::Capacity)?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FilesystemLabel(String);

impl TryFrom<String> for FilesystemLabel {
    type Error = ValueError;

    fn try_from(label: String) -> Result<Self, Self::Error> {
        if label.len() > 16 || label.contains('\0') {
            return Err(ValueError::Label);
        }
        Ok(Self(label))
    }
}

impl AsRef<str> for FilesystemLabel {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandTimeout(Duration);

impl CommandTimeout {
    pub fn duration(self) -> Duration {
        self.0
    }
}

impl Default for CommandTimeout {
    fn default() -> Self {
        Self(Duration::from_secs(300))
    }
}

impl FromStr for CommandTimeout {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let quantity = Quantity::try_from(text)?;
        let factor = match quantity.unit {
            "ms" => 1,
            "s" => 1000,
            "m" => 60 * 1000,
            "h" => 60 * 60 * 1000,
            _ => return Err(ValueError::Syntax),
        };
        let millis = quantity
            .integer
            .get()
            .checked_mul(factor)
            .ok_or(ValueError::Overflow)?;
        let duration = Duration::from_millis(millis);
        // A parsed timeout must also be representable as an execution deadline.
        std::time::Instant::now()
            .checked_add(duration)
            .ok_or(ValueError::Overflow)?;
        Ok(Self(duration))
    }
}
