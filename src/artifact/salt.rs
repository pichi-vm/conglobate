// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::SaltError;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

const MAX_BYTES: usize = 256;

/// Verity salt containing the 32-byte parent-root prefix, encoded as hex on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Salt(Vec<u8>);

impl TryFrom<Vec<u8>> for Salt {
    type Error = SaltError;
    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        if !(32..=MAX_BYTES).contains(&bytes.len()) {
            return Err(SaltError::Length(bytes.len()));
        }
        Ok(Self(bytes))
    }
}

impl AsRef<[u8]> for Salt {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl FromStr for Salt {
    type Err = SaltError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() > 2 * MAX_BYTES {
            return Err(SaltError::Length(text.len().div_ceil(2)));
        }
        hex::decode(text)?.try_into()
    }
}

impl fmt::Display for Salt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&hex::encode(&self.0))
    }
}

impl Serialize for Salt {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Salt {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::{Salt, SaltError};

    #[test]
    fn rejects_oversized_text_before_hex_decoding() {
        let maximum: Salt = "00".repeat(256).parse().unwrap();
        assert_eq!(maximum.as_ref().len(), 256);
        // Invalid hex would produce Hex if decoding ran before the size check.
        for length in [513, 514] {
            assert!(matches!(
                "g".repeat(length).parse::<Salt>(),
                Err(SaltError::Length(257))
            ));
        }
    }
}
