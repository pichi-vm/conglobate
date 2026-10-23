// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Committed top-level index pins. No network resolution or cache mutation.
use crate::{
    document::YamlDocument,
    image::{IdentityError, ImageReference, IndexDigest},
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("invalid lock YAML: {0}")]
    Document(#[from] serde_saphyr::Error),
    #[error("unsupported lock version {0}")]
    Version(u64),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("missing image pin: {0}")]
    Missing(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LockDocument {
    version: u64,
    images: BTreeMap<String, ImageDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ImageDocument {
    index: String,
}

#[derive(Debug)]
pub struct ImageLock {
    images: BTreeMap<ImageReference, IndexDigest>,
}

impl TryFrom<YamlDocument<'_>> for ImageLock {
    type Error = LockError;
    fn try_from(document: YamlDocument<'_>) -> Result<Self, Self::Error> {
        let raw: LockDocument = document.decode()?;
        if raw.version != 1 {
            return Err(LockError::Version(raw.version));
        }
        let images = raw
            .images
            .into_iter()
            .map(|(name, image)| Ok((name.parse()?, image.index.parse()?)))
            .collect::<Result<_, IdentityError>>()?;
        Ok(Self { images })
    }
}

impl ImageLock {
    pub fn resolve(&self, reference: &ImageReference) -> Result<IndexDigest, LockError> {
        self.images
            .get(reference)
            .copied()
            .ok_or_else(|| LockError::Missing(reference.as_ref().to_owned()))
    }
}
