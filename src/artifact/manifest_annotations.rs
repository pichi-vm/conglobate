// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::CarapaceVerity;
use serde::{Deserialize, Serialize};

/// Typed view of manifest annotations; assembly preserves the original JSON.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestAnnotations {
    /// Carapace identity and reconstruction parameters.
    #[serde(flatten)]
    pub verity: CarapaceVerity,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::Digest;
    use serde_json::json;

    #[test]
    fn decode_applies_defaults_and_accepts_native_spellings() {
        let empty: ManifestAnnotations = serde_json::from_str("{}").unwrap();
        assert_eq!(empty, ManifestAnnotations::default());
        assert_eq!(serde_json::to_value(empty).unwrap(), json!({}));
        let decoded: ManifestAnnotations = serde_json::from_value(json!({
            "dev.pichi.carapace.verity.hash": format!("sha256:{}", "AB".repeat(32)),
            "dev.pichi.carapace.verity.data-block-size": "02048",
            "dev.pichi.carapace.verity.hash-block-size": "+8192",
            "dev.pichi.carapace.verity.version": "01",
            "example.org/source": "retained in raw JSON"
        }))
        .unwrap();
        assert_eq!(decoded.verity.root, Some(Digest::Sha256([0xab; 32])));
        assert_eq!(decoded.verity.data_block_size.bytes().get(), 2048);
        assert_eq!(decoded.verity.hash_block_size.bytes().get(), 8192);
        let encoded = serde_json::to_value(decoded).unwrap();
        assert!(encoded.get("dev.pichi.carapace.verity.version").is_none());
        assert!(encoded.get("example.org/source").is_none());
    }

    #[test]
    fn decode_rejects_invalid_scalars() {
        for (key, value) in [
            ("hash", "ab".repeat(32)),
            ("hash", format!("sha512:{}", "ab".repeat(64))),
            ("data-block-size", "0".into()),
            ("data-block-size", "1000".into()),
            ("hash-block-size", "256".into()),
            ("hash-block-size", "1048576".into()),
            ("version", "2".into()),
        ] {
            let value = json!({ format!("dev.pichi.carapace.verity.{key}"): value });
            assert!(
                serde_json::from_value::<ManifestAnnotations>(value).is_err(),
                "{key}"
            );
        }
        assert!(
            serde_json::from_value::<ManifestAnnotations>(json!({
                "dev.pichi.carapace.verity.hash": 1
            }))
            .is_err()
        );
    }

    #[test]
    fn decode_ignores_unknown_fields_and_accepts_absent_root() {
        let decoded: ManifestAnnotations = serde_json::from_value(json!({
            "dev.pichi.carapace.verity.hash": null,
            "dev.pichi.carapace.verity.algo": "sha512",
            "dev.pichi.carapace.verity.hash-type": "0",
            "example.org/source": {"unknown": true}
        }))
        .unwrap();
        assert_eq!(decoded, ManifestAnnotations::default());
    }
}
