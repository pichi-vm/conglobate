// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{BuildProvenance, CarapaceVerity, OciAnnotations};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Typed manifest annotations, flattened into the OCI string-valued map.
/// Unknown keys are ignored, not captured for reserialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "BTreeMap<String, String>")]
pub struct ManifestAnnotations {
    /// Required captured build inputs.
    #[serde(flatten)]
    pub build: BuildProvenance,
    /// Chain-wide verity identity and defaulted reconstruction parameters.
    #[serde(flatten)]
    pub verity: CarapaceVerity,
    /// Optional informational OCI metadata.
    #[serde(flatten)]
    pub oci: OciAnnotations,
}

impl TryFrom<BTreeMap<String, String>> for ManifestAnnotations {
    type Error = serde::de::value::Error;

    fn try_from(annotations: BTreeMap<String, String>) -> Result<Self, Self::Error> {
        use serde::de::{Error as _, value::MapDeserializer};
        for key in [
            "dev.pichi.carapace.verity.hash-type",
            "dev.pichi.carapace.verity.algo",
        ] {
            if annotations.contains_key(key) {
                return Err(Self::Error::custom(format_args!(
                    "unsupported annotation: {key}"
                )));
            }
        }
        #[derive(Deserialize)]
        struct Wire {
            #[serde(flatten)]
            build: BuildProvenance,
            #[serde(flatten)]
            verity: CarapaceVerity,
            #[serde(flatten)]
            oci: OciAnnotations,
        }
        let wire = Wire::deserialize(MapDeserializer::new(annotations.into_iter()))?;
        Ok(Self {
            build: wire.build,
            verity: wire.verity,
            oci: wire.oci,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ManifestAnnotations;
    use crate::artifact::Commit;
    use serde_json::{Value, json};

    fn build() -> Value {
        json!({
            "dev.pichi.build.commit": format!("sha1:{}", "12".repeat(20)),
            "dev.pichi.build.context": ".",
            "dev.pichi.build.plan": "plan.yaml",
        })
    }

    #[test]
    fn required_build_inputs_and_optional_hints_round_trip() {
        let minimal = build();
        let value: ManifestAnnotations = serde_json::from_value(minimal.clone()).unwrap();
        assert_eq!(*value.build.commit, Commit::Sha1([0x12; 20]));
        assert!(value.build.lock.is_none());
        assert!(value.build.location.is_none());
        assert!(!*value.build.bootstrapped);
        assert_eq!(serde_json::to_value(value).unwrap(), minimal);

        let mut complete = minimal;
        complete["dev.pichi.build.commit"] = format!("sha256:{}", "ab".repeat(32)).into();
        complete["dev.pichi.build.lock"] = "lock.yaml".into();
        complete["dev.pichi.build.location"] = "https://example.org/source".into();
        complete["dev.pichi.build.bootstrapped"] = "true".into();
        complete["org.opencontainers.image.created"] = "2026-11-20T12:00:00Z".into();
        let value: ManifestAnnotations = serde_json::from_value(complete.clone()).unwrap();
        assert_eq!(*value.build.commit, Commit::Sha256([0xab; 32]));
        assert_eq!(serde_json::to_value(value).unwrap(), complete);
    }

    #[test]
    fn retired_verity_annotations_are_not_supported() {
        for (key, value) in [("hash-type", "1"), ("algo", "sha256")] {
            let mut fields = build();
            fields[format!("dev.pichi.carapace.verity.{key}")] = value.into();
            assert!(serde_json::from_value::<ManifestAnnotations>(fields).is_err());
        }
    }

    #[test]
    fn missing_or_malformed_build_fields_fail_at_deserialization() {
        for key in ["commit", "context", "plan"] {
            let mut missing = build();
            missing
                .as_object_mut()
                .unwrap()
                .remove(&format!("dev.pichi.build.{key}"));
            assert!(serde_json::from_value::<ManifestAnnotations>(missing).is_err());
        }
        for key in [
            "dev.pichi.build.lock",
            "dev.pichi.carapace.verity.hash",
            "unknown",
        ] {
            let mut fields = build();
            fields[key] = Value::Null;
            assert!(serde_json::from_value::<ManifestAnnotations>(fields).is_err());
        }
        for commit in [
            "sha1:1234".into(),
            format!("sha1:{}", "AB".repeat(20)),
            format!("sha256:{}", "ab".repeat(20)),
            format!("sha512:{}", "ab".repeat(64)),
        ] {
            let mut fields = build();
            fields["dev.pichi.build.commit"] = commit.into();
            assert!(serde_json::from_value::<ManifestAnnotations>(fields).is_err());
        }
        for (key, value) in [
            ("dev.pichi.build.bootstrapped", "yes"),
            ("org.opencontainers.image.created", "yesterday"),
        ] {
            let mut fields = build();
            fields[key] = value.into();
            assert!(serde_json::from_value::<ManifestAnnotations>(fields).is_err());
        }
    }
}
