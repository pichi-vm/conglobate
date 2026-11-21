// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::optional_build_provenance::OptionalBuildProvenance;
use super::{BuildProvenance, CarapaceVerity, OciAnnotations};
use serde::{Deserialize, Serialize};

/// Typed view of manifest annotations; assembly preserves the original JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestAnnotations {
    /// Informational OCI metadata, independent of platform selection.
    #[serde(flatten)]
    pub oci: OciAnnotations,
    /// Complete source binding for a build; absent on raw imports.
    #[serde(flatten, deserialize_with = "OptionalBuildProvenance::deserialize")]
    pub build: Option<BuildProvenance>,
    /// Carapace identity and reconstruction parameters.
    #[serde(flatten)]
    pub verity: CarapaceVerity,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::Digest;
    use serde_json::json;

    fn with_provenance(extra: serde_json::Value) -> serde_json::Value {
        let mut values = json!({
            "dev.pichi.build.commit": format!("sha1:{}", "ab".repeat(20)),
            "dev.pichi.build.context": ".",
            "dev.pichi.build.plan": "plan.yaml"
        });
        values
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        values
    }

    #[test]
    fn decode_applies_defaults_and_accepts_native_spellings() {
        let minimal = with_provenance(json!({}));
        let decoded: ManifestAnnotations = serde_json::from_value(minimal.clone()).unwrap();
        assert_eq!(decoded.verity, CarapaceVerity::default());
        assert_eq!(serde_json::to_value(decoded).unwrap(), minimal);
        let decoded: ManifestAnnotations = serde_json::from_value(with_provenance(json!({
            "dev.pichi.carapace.verity.hash": format!("sha256:{}", "AB".repeat(32)),
            "dev.pichi.carapace.verity.data-block-size": "02048",
            "dev.pichi.carapace.verity.hash-block-size": "+8192",
            "dev.pichi.carapace.verity.version": "01",
            "example.org/source": "retained in raw JSON"
        })))
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
                serde_json::from_value::<ManifestAnnotations>(with_provenance(value)).is_err(),
                "{key}"
            );
        }
        assert!(
            serde_json::from_value::<ManifestAnnotations>(with_provenance(json!({
                "dev.pichi.carapace.verity.hash": 1
            })))
            .is_err()
        );
    }

    #[test]
    fn decode_ignores_unknown_fields_and_accepts_absent_root() {
        let decoded: ManifestAnnotations = serde_json::from_value(with_provenance(json!({
            "dev.pichi.carapace.verity.hash": null,
            "dev.pichi.carapace.verity.algo": "sha512",
            "dev.pichi.carapace.verity.hash-type": "0",
            "example.org/source": {"unknown": true}
        })))
        .unwrap();
        assert_eq!(decoded.verity, CarapaceVerity::default());
    }

    #[test]
    fn provenance_requires_source_facts_without_inventing_optional_values() {
        let value = with_provenance(json!({}));
        let imported: ManifestAnnotations = serde_json::from_value(json!({})).unwrap();
        assert!(imported.build.is_none());
        assert_eq!(serde_json::to_value(imported).unwrap(), json!({}));
        for key in ["commit", "context", "plan"] {
            let key = format!("dev.pichi.build.{key}");
            let mut missing = value.clone();
            missing.as_object_mut().unwrap().remove(&key);
            assert!(serde_json::from_value::<ManifestAnnotations>(missing).is_err());
        }
        let decoded: ManifestAnnotations = serde_json::from_value(value).unwrap();
        let build = decoded.build.unwrap();
        assert!(build.lock.is_none());
        assert!(build.source.is_none());
        assert!(!*build.bootstrapped);
        for (key, value) in [("commit", "sha1:abcd"), ("bootstrapped", "yes")] {
            assert!(
                serde_json::from_value::<ManifestAnnotations>(with_provenance(json!({
                    format!("dev.pichi.build.{key}"): value
                })))
                .is_err()
            );
        }
    }

    #[test]
    fn present_provenance_cannot_be_discarded_as_absent() {
        let complete = with_provenance(json!({
            "dev.pichi.build.lock": "lock.yaml",
            "org.opencontainers.image.source": "https://example.org/source",
            "dev.pichi.build.bootstrapped": "false"
        }));
        for (key, value) in complete.as_object().unwrap() {
            assert!(serde_json::from_value::<ManifestAnnotations>(json!({key: value})).is_err());
            let mut invalid = complete.clone();
            invalid[key] = json!(123);
            assert!(serde_json::from_value::<ManifestAnnotations>(invalid).is_err());
            // Preserve duplicate-key rejection, including default/optional fields.
            let encoded = serde_json::to_string(&complete).unwrap();
            let duplicate = format!("{{{}:{},{}", json!(key), value, &encoded[1..]);
            assert!(serde_json::from_str::<ManifestAnnotations>(&duplicate).is_err());
            assert!(
                serde_json::from_value::<ManifestAnnotations>(json!({key: null}))
                    .unwrap()
                    .build
                    .is_none()
            );
        }
        let absent = json!({"example.org/metadata": {"arbitrary": true}});
        assert!(
            serde_json::from_value::<ManifestAnnotations>(absent)
                .unwrap()
                .build
                .is_none()
        );
        let present: ManifestAnnotations = serde_json::from_value(complete.clone()).unwrap();
        assert!(present.build.is_some());
        let mut expected = complete;
        expected
            .as_object_mut()
            .unwrap()
            .remove("dev.pichi.build.bootstrapped");
        assert_eq!(serde_json::to_value(present).unwrap(), expected);
    }

    #[test]
    fn oci_annotations_preserve_informational_strings() {
        let values = with_provenance(json!({
            "org.opencontainers.image.created": "2026-05-06T14:32:00+05:30",
            "org.opencontainers.image.source": "https://example.org/source",
            "org.opencontainers.image.architecture": "informational-hint"
        }));
        let decoded: ManifestAnnotations = serde_json::from_value(values.clone()).unwrap();
        assert_eq!(
            decoded.oci.created.as_deref(),
            Some("2026-05-06T14:32:00+05:30")
        );
        assert_eq!(
            decoded.build.as_ref().unwrap().source.as_deref(),
            Some("https://example.org/source")
        );
        assert_eq!(
            decoded.oci.architecture.as_deref(),
            Some("informational-hint")
        );
        assert_eq!(serde_json::to_value(decoded).unwrap(), values);
        assert!(
            serde_json::from_value::<ManifestAnnotations>(with_provenance(json!({
                "org.opencontainers.image.created": 123
            })))
            .is_err()
        );
    }
}
