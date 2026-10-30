// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Committed top-level index pins. No network resolution or cache mutation.
use crate::{
    document::YamlDocument,
    image::{IdentityError, ImageReference, IndexDigest},
};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub(crate) enum LockError {
    #[error("invalid lock YAML: {0}")]
    Document(#[from] serde_saphyr::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("missing image pin: {0}")]
    Missing(String),
}

#[derive(Debug)]
pub(crate) struct ImageLock {
    images: BTreeMap<ImageReference, IndexDigest>,
}

impl TryFrom<YamlDocument<'_>> for ImageLock {
    type Error = LockError;
    fn try_from(document: YamlDocument<'_>) -> Result<Self, Self::Error> {
        let raw: crate::contract::build::Lock = document.decode()?;
        let images = raw
            .images
            .iter()
            .map(|(reference, pin)| {
                Ok((
                    reference.parse()?,
                    IndexDigest::from_bytes(pin.index.as_sha256_array()),
                ))
            })
            .collect::<Result<_, IdentityError>>()?;
        Ok(Self { images })
    }
}

impl ImageLock {
    pub(crate) fn resolve(&self, reference: &ImageReference) -> Result<IndexDigest, LockError> {
        self.images
            .get(reference)
            .copied()
            .ok_or_else(|| LockError::Missing(reference.as_ref().to_owned()))
    }
}
