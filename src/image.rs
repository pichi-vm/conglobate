// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Image names and content identities are distinct from verified image handles.
use std::str::FromStr;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub(crate) enum IdentityError {
    #[error("image reference must be a fully qualified OCI reference with a registry host")]
    Reference,
    #[error("expected sha256: followed by 64 hexadecimal digits")]
    Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ImageReference(String);

impl FromStr for ImageReference {
    type Err = IdentityError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        text.parse::<crate::contract::recipe::Remote>()
            .map_err(|_| IdentityError::Reference)?;
        Ok(Self(text.to_owned()))
    }
}

impl AsRef<str> for ImageReference {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct IndexDigest([u8; 32]);

impl IndexDigest {
    pub(crate) fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FromStr for IndexDigest {
    type Err = IdentityError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let encoded = text.strip_prefix("sha256:").ok_or(IdentityError::Digest)?;
        let mut bytes = [0; 32];
        hex::decode_to_slice(encoded, &mut bytes).map_err(|_| IdentityError::Digest)?;
        Ok(Self(bytes))
    }
}
