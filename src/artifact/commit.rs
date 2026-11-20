// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{fmt, str::FromStr};

/// A full Git commit identity, including its object format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Commit {
    /// A SHA-1 Git object ID.
    Sha1([u8; 20]),
    /// A SHA-256 Git object ID.
    Sha256([u8; 32]),
}

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sha1(bytes) => write!(f, "sha1:{}", hex::encode(bytes)),
            Self::Sha256(bytes) => write!(f, "sha256:{}", hex::encode(bytes)),
        }
    }
}

impl FromStr for Commit {
    type Err = &'static str;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (algorithm, value) = text.split_once(':').ok_or("missing Git object format")?;
        match algorithm {
            "sha1" => {
                let mut bytes = [0; 20];
                hex::decode_to_slice(value, &mut bytes).map_err(|_| "invalid SHA-1 commit")?;
                Ok(Self::Sha1(bytes))
            }
            "sha256" => {
                let mut bytes = [0; 32];
                hex::decode_to_slice(value, &mut bytes).map_err(|_| "invalid SHA-256 commit")?;
                Ok(Self::Sha256(bytes))
            }
            _ => Err("unsupported Git object format"),
        }
    }
}
