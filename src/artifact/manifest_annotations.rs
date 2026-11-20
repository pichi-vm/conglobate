// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::CarapaceVerity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Typed manifest annotations, flattened into the OCI string-valued map.
/// Other producer annotations remain available until modeled separately.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(try_from = "BTreeMap<String, String>")]
pub struct ManifestAnnotations {
    /// Chain-wide verity identity and defaulted reconstruction parameters.
    #[serde(flatten)]
    pub verity: CarapaceVerity,
    /// Unrelated producer and OCI metadata.
    #[serde(flatten)]
    pub other: BTreeMap<String, String>,
}

impl ManifestAnnotations {
    pub(super) fn is_empty(&self) -> bool {
        self.verity == CarapaceVerity::default() && self.other.is_empty()
    }
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
            verity: CarapaceVerity,
            #[serde(flatten)]
            other: BTreeMap<String, String>,
        }
        // OCI annotations are strings. Decode that boundary before flattened
        // optional fields so explicit null cannot be mistaken for absence.
        let wire = Wire::deserialize(MapDeserializer::new(annotations.into_iter()))?;
        Ok(Self {
            verity: wire.verity,
            other: wire.other,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ManifestAnnotations;
    #[test]
    fn hash_type_annotation_is_not_supported() {
        for value in ["0", "1", "normal", ""] {
            let fields = serde_json::json!({"dev.pichi.carapace.verity.hash-type": value});
            assert!(serde_json::from_value::<ManifestAnnotations>(fields).is_err());
        }
    }
}
