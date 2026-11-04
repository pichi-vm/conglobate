// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Committed top-level index pins. No network resolution or cache mutation.
use crate::{
    document::YamlDocument,
    image::{ImageReference, IndexDigest},
};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub(crate) enum LockError {
    #[error("invalid lock YAML: {0}")]
    Document(#[from] serde_saphyr::Error),
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
        let raw: crate::contract::plan::Lock = document.decode()?;
        let images = raw
            .images
            .iter()
            .map(|(reference, pin)| {
                (
                    ImageReference::from(reference),
                    IndexDigest::from_bytes(pin.as_sha256_array()),
                )
            })
            .collect();
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

#[cfg(test)]
mod tests {
    use crate::{document::YamlDocument, lock::ImageLock};

    #[test]
    fn pins_are_exact_top_level_identities() {
        let text = format!(
            "version: 1\nimages:\n  example.test/base:1: sha256:{}\n",
            "ab".repeat(32)
        );
        let lock = ImageLock::try_from(YamlDocument::from(text.as_str())).unwrap();
        assert_eq!(
            lock.resolve(&"example.test/base:1".parse().unwrap())
                .unwrap()
                .bytes(),
            &[0xab; 32]
        );
        assert!(
            lock.resolve(&"example.test/missing:1".parse().unwrap())
                .is_err()
        );
    }

    #[test]
    fn metadata_lock_pins_the_documented_fixture_index() {
        use sha2::{Digest as _, Sha256};
        let text = include_str!("../tests/fixtures/metadata-lock.yaml");
        let bytes = text
            .lines()
            .find_map(|line| line.strip_prefix("# index-bytes: "))
            .expect("documented fixture index");
        let mut hash = Sha256::new();
        hash.update(bytes.as_bytes());
        let lock = ImageLock::try_from(YamlDocument::from(text)).unwrap();
        let pin = lock
            .resolve(&"qualification.local/metadata:1".parse().unwrap())
            .unwrap();
        assert_eq!(pin.bytes().as_slice(), hash.finalize().as_slice());
    }

    #[test]
    fn duplicate_references_and_unknown_fields_are_rejected() {
        let pin = format!("sha256:{}", "00".repeat(32));
        for text in [
            format!(
                "version: 1\nimages:\n  registry.test/base:1: {pin}\n  registry.test/base:1: {pin}"
            ),
            format!("version: 1\nversion: 1\nimages: {{registry.test/base:1: {pin}}}"),
            "version: 1\nimages: {}\nextra: ignored".into(),
            "version: 1\nimages: null".into(),
            format!("version: 1\nimages: {{base: {pin}}}"),
            "version: 2\nimages: {}".into(),
            "version: 1\nimages: {registry.test/base:1: latest}".into(),
            "version: 1\nimages: {registry.test/base:1: {index: sha256:00, manifest: other}}"
                .into(),
        ] {
            assert!(
                ImageLock::try_from(YamlDocument::from(text.as_str())).is_err(),
                "{text}"
            );
        }
    }
}
