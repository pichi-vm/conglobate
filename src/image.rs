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
        crate::contract::plan::Remote::new(text.to_owned())
            .map(|remote| Self::from(&remote))
            .ok_or(IdentityError::Reference)
    }
}

impl From<&crate::contract::plan::Remote> for ImageReference {
    fn from(remote: &crate::contract::plan::Remote) -> Self {
        Self(remote.to_string())
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
    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl FromStr for IndexDigest {
    type Err = IdentityError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let digest: oci_spec::image::Digest = text.parse().map_err(|_| IdentityError::Digest)?;
        if digest.algorithm() != &oci_spec::image::DigestAlgorithm::Sha256 {
            return Err(IdentityError::Digest);
        }
        let mut bytes = [0; 32];
        hex::decode_to_slice(digest.digest(), &mut bytes).map_err(|_| IdentityError::Digest)?;
        Ok(Self(bytes))
    }
}
