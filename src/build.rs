// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Inputs shared by the host and builder. Transport and resolution are external.

use crate::artifact::Digest;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::collections::BTreeMap;

/// Current image-lock wire version.
pub const LOCK_VERSION: u32 = 1;

/// Remote image references pinned to immutable OCI indexes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    #[serde(deserialize_with = "lock_version")]
    pub version: u32,
    #[serde(deserialize_with = "image_pins")]
    pub images: BTreeMap<String, ImagePin>,
}

impl Default for Lock {
    fn default() -> Self {
        Self {
            version: LOCK_VERSION,
            images: BTreeMap::new(),
        }
    }
}

/// The index identifies all supported architectures for one input image.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImagePin {
    pub index: Digest,
}

fn image_pins<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, ImagePin>, D::Error> {
    Option::<BTreeMap<String, ImagePin>>::deserialize(deserializer)?
        .ok_or_else(|| de::Error::custom("images must be a non-null map"))
}

fn lock_version<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    match u32::deserialize(deserializer)? {
        LOCK_VERSION => Ok(LOCK_VERSION),
        _ => Err(de::Error::custom("unsupported lock version")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_preserves_reference_keys_and_digest_strings() {
        let reference = "registry.example/base:1";
        let mut lock = Lock::default();
        lock.images.insert(
            reference.into(),
            ImagePin {
                index: Digest::Sha256([0x11; 32]),
            },
        );
        let value = serde_json::to_value(&lock).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(
            value["images"][reference]["index"],
            format!("sha256:{}", "11".repeat(32))
        );
        assert_eq!(serde_json::from_value::<Lock>(value).unwrap(), lock);
    }

    #[test]
    fn lock_rejects_invalid_shape_version_and_digest() {
        for text in [
            r#"{"version":2,"images":{}}"#,
            r#"{"version":1,"images":null}"#,
            r#"{"version":1,"images":{},"extra":true}"#,
            r#"{"version":1,"images":{"registry.example/base:1":{"index":"sha256:01"}}}"#,
            r#"{"version":1,"images":{"registry.example/base:1":{"index":"md5:01"}}}"#,
        ] {
            assert!(serde_json::from_str::<Lock>(text).is_err(), "{text}");
        }
        assert_eq!(
            serde_json::from_str::<Lock>(r#"{"version":1,"images":{}}"#).unwrap(),
            Lock::default()
        );
    }
}
