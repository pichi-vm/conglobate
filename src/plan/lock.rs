// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Inputs shared by the host and builder. Transport and resolution are external.

use crate::artifact::Digest;
use crate::plan::{Remote, Version};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Remote image references pinned to immutable OCI indexes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    pub version: Version<1>,
    /// Each digest identifies a multi-architecture OCI index.
    pub images: BTreeMap<Remote, Digest>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_preserves_reference_keys_and_digest_strings() {
        let reference = "registry.example/base:1";
        let mut lock = Lock::default();
        lock.images.insert(
            Remote::new(reference.into()).unwrap(),
            Digest::Sha256([0x11; 32]),
        );
        let value = serde_json::to_value(&lock).unwrap();
        assert_eq!(value["version"], 1);
        assert_eq!(
            value["images"][reference],
            format!("sha256:{}", "11".repeat(32))
        );
        assert_eq!(serde_json::from_value::<Lock>(value).unwrap(), lock);
    }

    #[test]
    fn lock_rejects_invalid_shape_version_and_digest() {
        for text in [
            r#"{"images":{}}"#,
            r#"{"version":2,"images":{}}"#,
            r#"{"version":1,"images":null}"#,
            r#"{"version":1,"images":{},"extra":true}"#,
            r#"{"version":1,"images":{"registry.example/base:1":"sha256:01"}}"#,
            r#"{"version":1,"images":{"registry.example/base:1":"md5:01"}}"#,
        ] {
            assert!(serde_json::from_str::<Lock>(text).is_err(), "{text}");
        }
        assert_eq!(
            serde_json::from_str::<Lock>(r#"{"version":1,"images":{}}"#).unwrap(),
            Lock::default()
        );
    }

    #[cfg(feature = "cli")]
    #[test]
    fn yaml_maps_preserve_null_rejection() {
        let lock: Lock = serde_saphyr::from_str("version: 1\nimages: {}\n").unwrap();
        assert_eq!(lock, Lock::default());
        assert!(serde_saphyr::from_str::<Lock>("version: 1\nimages: null\n").is_err());
    }

    #[test]
    fn lock_keys_require_fully_qualified_references() {
        for reference in ["base", "team/base", "team/base:1"] {
            let value = serde_json::json!({
                "version": 1,
                "images": {reference: format!("sha256:{}", "11".repeat(32))}
            });
            assert!(
                serde_json::from_value::<Lock>(value).is_err(),
                "{reference}"
            );
        }
    }

    #[test]
    fn implicit_latest_lock_key_matches_an_explicit_reference() {
        let value = serde_json::json!({
            "version": 1,
            "images": {"registry.example/base": format!("sha256:{}", "11".repeat(32))}
        });
        let lock: Lock = serde_json::from_value(value).unwrap();
        let reference = Remote::new("registry.example/base:latest".into()).unwrap();
        assert_eq!(lock.images[&reference], Digest::Sha256([0x11; 32]));
    }
}
