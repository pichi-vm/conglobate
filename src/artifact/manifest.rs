// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! `Manifest` typed schema (Phase 42 FORMAT-01..03). Per D-03, manifests
//! are content-addressed JSON stored in the blob store; this type provides
//! parse / serialise / validate / self-digest helpers.
//!
//! Phase 42 locks the schema: typed `layers: Vec<Layer>` (discriminated
//! by required fixed `mediaType` fields), `ConfigDescriptor` for the Pichi launch
//! contract, with field types and `validate()` enforcing the format rules.

use super::Media;
use super::{
    AsString, BootConfigDescriptor, ConfigDescriptor, Digest, ManifestAnnotations, MediaType,
    PichiArtifact,
};

use crate::plan::Version;
use devmap_verity::Salt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Algorithm-qualified SHA-256 root of the final scute.
pub const CARAPACE_ROOT_ANNOTATION: &str = "dev.pichi.carapace.verity.hash";

impl Media for Manifest {
    const MEDIA_TYPE: &'static str = "application/vnd.oci.image.manifest.v1+json";
}

/// pichi OCI artifact manifest (OCI Image Spec 1.1).
///
/// Schema invariants (per CONTEXT.md D-03 / D-07), enforced by field types and
/// [`Manifest::validate`]:
/// - Top-level `artifact_type` is fixed by [`MediaType<PichiArtifact>`].
/// - `config` references a Pichi launch-contract blob.
/// - Scute layers MUST appear in chain order, from base to tip.
/// - At most ONE PMI, raw or compressed (D-03 — zero pmi = base/non-bootable, one pmi = bootable).
/// - Top-level annotations MUST include one chain-tip root when scutes exist.
///   Reconstruction parameters are optional with defined defaults.
/// - Every scute layer MUST carry a `dev.pichi.scute.verity.salt` annotation
///   parseable as hex (D-05 / D-07 rule 4-5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    /// Always `2` for OCI 1.1.
    #[serde(rename = "schemaVersion")]
    pub schema_version: Version<2>,
    /// OCI image manifest media type (`application/vnd.oci.image.manifest.v1+json`).
    #[serde(rename = "mediaType")]
    pub media_type: MediaType<Manifest>,
    /// Fixed Pichi artifact kind, encoded as the OCI `artifactType` string.
    #[serde(rename = "artifactType")]
    pub artifact_type: MediaType<PichiArtifact>,
    /// Descriptor of the Pichi launch-contract blob.
    pub config: ConfigDescriptor,
    /// Typed layer descriptors. Scutes appear in base-to-tip chain order;
    /// PMI and boot configuration descriptors are outside that order.
    pub layers: Vec<Layer>,
    /// Typed annotation view. Verity parameters have native defaults;
    /// unrelated annotations remain in the original manifest bytes.
    pub annotations: ManifestAnnotations,
}

/// Layer variants discriminate on their required fixed `mediaType` fields.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
#[allow(
    clippy::large_enum_variant,
    reason = "Keep descriptors inline without per-layer allocations"
)]
pub enum Layer {
    Scute(ScuteDescriptor),
    Pmi(PmiDescriptor),
    BootConfig(BootConfigDescriptor),
}

impl Layer {
    /// The content identity, regardless of layer encoding.
    pub fn digest(&self) -> &Digest {
        match self {
            Self::Scute(d) => &d.digest,
            Self::Pmi(d) => &d.digest,
            Self::BootConfig(d) => &d.digest,
        }
    }

    /// Return the descriptor size regardless of variant.
    pub fn size(&self) -> u64 {
        match self {
            Self::Scute(d) => d.size,
            Self::Pmi(d) => d.size,
            Self::BootConfig(d) => d.size,
        }
    }

    /// Return `true` if this layer's mediaType ends in `+zstd`. Used by
    /// Phase 44 cmd::pull's pipeline composer to decide whether to wrap
    /// the `BlobStore`-side sink in a `ZstdDecodeWriter` (Pitfall 5/12).
    pub fn is_zstd_variant(&self) -> bool {
        match self {
            Self::Scute(d) => matches!(d.media_type, super::Compressable::Zstd(_)),
            Self::Pmi(d) => matches!(d.media_type, super::Compressable::Zstd(_)),
            Self::BootConfig(_) => true,
        }
    }

    /// Return the descriptor's fixed media type.
    pub fn media_type_str(&self) -> &'static str {
        match self {
            Self::Scute(d) => match d.media_type {
                super::Compressable::Raw(_) => ScuteDescriptor::MEDIA_TYPE,
                super::Compressable::Zstd(_) => {
                    crate::artifact::Zstd::<ScuteDescriptor>::MEDIA_TYPE
                }
            },
            Self::Pmi(d) => match d.media_type {
                super::Compressable::Raw(_) => PmiDescriptor::MEDIA_TYPE,
                super::Compressable::Zstd(_) => crate::artifact::Zstd::<PmiDescriptor>::MEDIA_TYPE,
            },
            Self::BootConfig(_) => BootConfigDescriptor::MEDIA_TYPE,
        }
    }
}

/// Scute descriptor with an explicit raw or compressed transport.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScuteDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: super::Compressable<Self>,
    /// Content digest of the blob.
    pub digest: Digest,
    /// Byte size of the blob.
    pub size: u64,
    /// Per-scute annotations (D-05).
    pub annotations: ScuteAnnotations,
}

/// Per-scute annotations (D-05). The salt's prefix MUST equal the previous
/// scute's verity root for non-base scutes (carapace salt-chain binding —
/// not enforced here; chain validation is v0.9 Phase 46 carapace work).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScuteAnnotations {
    /// dm-verity header `data_blocks`: COW length rounded up to data blocks.
    /// Required, positive, and encoded as a decimal annotation string.
    #[serde(rename = "dev.pichi.scute.verity.data-blocks")]
    pub data_blocks: AsString<std::num::NonZeroU64>,
    /// Full salt as hex, including the parent-root prefix.
    #[serde(rename = "dev.pichi.scute.verity.salt")]
    pub salt: AsString<Salt>,
    /// Required SHA-256 of the exact decompressed bytes.
    #[serde(rename = "dev.pichi.scute.digest")]
    pub digest: Digest,
    /// Required exact decompressed byte count as a decimal annotation string.
    #[serde(rename = "dev.pichi.scute.size")]
    pub size: AsString<std::num::NonZeroU64>,
    /// Required SHA-256 of the complete generated verity file, including its
    /// header and padding. This is distinct from the verity root.
    #[serde(rename = "dev.pichi.scute.verity.digest")]
    pub verity_digest: Digest,
}

/// PMI layer descriptor. Digest and size identify the encoded blob bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PmiDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: super::Compressable<Self>,
    /// Content digest of the PMI blob.
    pub digest: Digest,
    /// Byte size of the PMI blob.
    pub size: u64,
}

/// Validation errors for [`Manifest::validate`] (per D-07).
#[derive(Debug, Error)]
pub enum ManifestValidationError {
    /// D-07 rule 3: required chain annotation key is absent.
    #[error("missing required chain annotation: {0}")]
    MissingChainAnnotation(&'static str),
    /// The chain root contradicts the manifest's scute structure.
    #[error("chain annotation {key} is malformed: {value}")]
    BadChainAnnotation {
        /// The annotation key.
        key: &'static str,
        /// The offending value.
        value: String,
    },
    /// D-07 rule 5: more than one PMI layer.
    #[error("artifact has more than one PMI layer (got {0}); per D-03 at most one is permitted")]
    MultiplePmi(usize),

    /// More than one boot configuration layer.
    #[error("artifact has more than one boot configuration layer (got {0})")]
    MultipleBootConfig(usize),

    /// Boot configuration cannot be distributed independently of firmware.
    #[error("boot configuration requires a PMI")]
    BootConfigWithoutPmi,

    /// Scute salt lacks the SHA-256 parent-root prefix.
    #[error("scute salt must contain a SHA-256 parent root: {0}")]
    BadSalt(String),

    /// The first scute must be the base; later scutes must have a parent.
    #[error("scutes must appear in base-to-tip order: {0}")]
    BadScuteOrder(Digest),

    /// A supplied content identity is malformed or contradicts scute geometry.
    #[error("scute annotation {key} is malformed: {value}")]
    BadScuteAnnotation {
        /// The annotation key.
        key: &'static str,
        /// The offending value.
        value: String,
    },

    /// Declared data-block count is zero or contradicts a raw blob's size.
    #[error("invalid data-block count for scute {0}")]
    BadScuteBlocks(Digest),
}

impl Manifest {
    /// The chain-tip commitment, present exactly when this artifact has scutes.
    /// Consumers must authenticate the manifest and
    /// verify scute reads against this root before using their contents.
    pub fn carapace_root(&self) -> Result<Option<[u8; 32]>, ManifestValidationError> {
        let root = self.annotations.verity.root.as_ref();
        if !self.has_scutes() && root.is_none() {
            return Ok(None);
        }
        let root = root.ok_or(ManifestValidationError::MissingChainAnnotation(
            CARAPACE_ROOT_ANNOTATION,
        ))?;
        if !self.has_scutes() || root.as_sha256_array() == [0; 32] {
            return Err(ManifestValidationError::BadChainAnnotation {
                key: CARAPACE_ROOT_ANNOTATION,
                value: root.to_string(),
            });
        }
        Ok(Some(root.as_sha256_array()))
    }

    /// Return `true` if this manifest carries at least one scute layer (i.e.
    /// it describes a carapace, so a verity root anchor is meaningful).
    fn has_scutes(&self) -> bool {
        self.layers.iter().any(|l| matches!(l, Layer::Scute(_)))
    }

    /// Validate manifest structure. Scute order starts with a zero-parent base;
    /// consumers must verify each subsequent parent against the preceding
    /// scute's content and the final root against the manifest commitment.
    ///
    /// # Errors
    /// Returns the first violated rule — does NOT collect all errors.
    pub fn validate(&self) -> Result<(), ManifestValidationError> {
        self.carapace_root()?;
        // Rule 5 (out of order — bail early on count): at most one PMI
        let pmi_count = self
            .layers
            .iter()
            .filter(|l| matches!(l, Layer::Pmi(_)))
            .count();
        if pmi_count > 1 {
            return Err(ManifestValidationError::MultiplePmi(pmi_count));
        }
        let boot_config_count = self
            .layers
            .iter()
            .filter(|layer| matches!(layer, Layer::BootConfig(_)))
            .count();
        if boot_config_count > 1 {
            return Err(ManifestValidationError::MultipleBootConfig(
                boot_config_count,
            ));
        }
        if boot_config_count != 0 && pmi_count == 0 {
            return Err(ManifestValidationError::BootConfigWithoutPmi);
        }
        // The salt's parent prefix identifies the base without reading blobs.
        // Full chain continuity is checked when consumers verify the scutes.
        let mut first_scute = true;
        for layer in &self.layers {
            let Layer::Scute(d) = layer else {
                continue;
            };
            let blocks = d.annotations.data_blocks.get();
            let block_size = self.annotations.verity.data_block_size.bytes().get();
            if blocks.checked_mul(u64::from(block_size)).is_none()
                || (!layer.is_zstd_variant() && blocks != d.size.div_ceil(u64::from(block_size)))
            {
                return Err(ManifestValidationError::BadScuteBlocks(d.digest.clone()));
            }
            if !layer.is_zstd_variant() && d.annotations.digest != d.digest {
                return Err(ManifestValidationError::BadScuteAnnotation {
                    key: "dev.pichi.scute.digest",
                    value: d.annotations.digest.to_string(),
                });
            }
            let size = d.annotations.size.get();
            if size.div_ceil(u64::from(block_size)) != blocks
                || (!layer.is_zstd_variant() && size != d.size)
            {
                return Err(ManifestValidationError::BadScuteAnnotation {
                    key: "dev.pichi.scute.size",
                    value: size.to_string(),
                });
            }
            let parent =
                d.annotations.salt.as_ref().get(..32).ok_or_else(|| {
                    ManifestValidationError::BadSalt(d.annotations.salt.to_string())
                })?;
            if first_scute != parent.iter().all(|byte| *byte == 0) {
                return Err(ManifestValidationError::BadScuteOrder(d.digest.clone()));
            }
            first_scute = false;
        }
        // Rule 6: every layer's mediaType is in the allowed set —
        // ENFORCED IMPLICITLY by `Layer`'s enum variants (an unknown
        // mediaType fails to deserialize before validate() is even called).
        Ok(())
    }
}

impl crate::artifact::Media for ScuteDescriptor {
    const MEDIA_TYPE: &'static str = "application/vnd.pichi.scute.v1";
}

impl crate::artifact::Media for PmiDescriptor {
    const MEDIA_TYPE: &'static str = "application/vnd.pichi.pmi.v1";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::PichiArtifact;
    use std::num::NonZeroU64;

    #[test]
    fn pmi_encodings_share_cardinality_and_bootconfig_rules() {
        let descriptor = PmiDescriptor {
            media_type: crate::artifact::Compressable::Raw(Default::default()),
            digest: Digest::Sha256([0x12; 32]),
            size: 73,
        };
        let variants = [
            Layer::Pmi(crate::artifact::PmiDescriptor {
                media_type: crate::artifact::Compressable::Raw(Default::default()),
                ..descriptor.clone()
            }),
            Layer::Pmi(crate::artifact::PmiDescriptor {
                media_type: crate::artifact::Compressable::Zstd(Default::default()),
                ..descriptor
            }),
        ];
        for (encoded, expected_type) in variants.iter().zip([
            "application/vnd.pichi.pmi.v1",
            "application/vnd.pichi.pmi.v1+zstd",
        ]) {
            let json = serde_json::to_value(encoded).unwrap();
            assert_eq!(json["mediaType"], expected_type);
            assert_eq!(serde_json::from_value::<Layer>(json).unwrap(), *encoded);
            assert_eq!(encoded.media_type_str(), expected_type);
            assert_eq!(encoded.is_zstd_variant(), expected_type.ends_with("+zstd"));
            assert_eq!(encoded.size(), 73);
            for root in [false, true] {
                let mut manifest = sample_base_manifest();
                if !root {
                    manifest.layers.clear();
                    manifest.annotations.verity = Default::default();
                }
                manifest.layers.push(encoded.clone());
                manifest
                    .layers
                    .push(Layer::BootConfig(BootConfigDescriptor {
                        media_type: Default::default(),
                        digest: Digest::Sha256([0x34; 32]),
                        size: 12,
                    }));
                manifest.validate().unwrap();
                for duplicate in &variants {
                    let mut two = manifest.clone();
                    two.layers.push(duplicate.clone());
                    assert!(matches!(
                        two.validate(),
                        Err(ManifestValidationError::MultiplePmi(2))
                    ));
                }
            }
        }
    }

    #[test]
    fn boot_config_is_a_compressed_component_requiring_one_pmi() {
        let config = Layer::BootConfig(BootConfigDescriptor {
            media_type: Default::default(),
            digest: Digest::Sha256([0x34; 32]),
            size: 73,
        });
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(
            json["mediaType"],
            "application/vnd.pichi.bootconfig.v1+zstd"
        );
        assert_eq!(json["size"], 73);
        assert_eq!(serde_json::from_value::<Layer>(json).unwrap(), config);
        assert!(config.is_zstd_variant());
        assert_eq!(config.media_type_str(), BootConfigDescriptor::MEDIA_TYPE);

        for root in [false, true] {
            let mut manifest = sample_base_manifest();
            if !root {
                manifest.layers.clear();
                manifest.annotations.verity = Default::default();
            }
            manifest.layers.push(config.clone());
            assert!(matches!(
                manifest.validate(),
                Err(ManifestValidationError::BootConfigWithoutPmi)
            ));
            manifest.layers.push(Layer::Pmi(PmiDescriptor {
                media_type: crate::artifact::Compressable::Raw(Default::default()),
                digest: Digest::Sha256([0x12; 32]),
                size: 4096,
            }));
            manifest.validate().unwrap();
            manifest.layers.push(config.clone());
            assert!(matches!(
                manifest.validate(),
                Err(ManifestValidationError::MultipleBootConfig(2))
            ));
        }
    }

    #[test]
    fn data_blocks_is_a_required_positive_decimal_annotation() {
        let annotations = ScuteAnnotations {
            digest: Digest::Sha256([0x11; 32]),
            size: NonZeroU64::new(1024).unwrap().into(),
            verity_digest: Digest::Sha256([0x22; 32]),
            salt: Salt::new(&[0]).unwrap().into(),
            data_blocks: std::num::NonZeroU64::new(2).unwrap().into(),
        };
        let json = serde_json::to_value(&annotations).unwrap();
        let mut missing = json.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("dev.pichi.scute.verity.data-blocks");
        assert!(serde_json::from_value::<ScuteAnnotations>(missing).is_err());
        assert_eq!(json["dev.pichi.scute.verity.data-blocks"], "2");
        assert_eq!(
            serde_json::from_value::<ScuteAnnotations>(json.clone()).unwrap(),
            annotations
        );
        for invalid in [
            serde_json::json!(4097),
            serde_json::json!(""),
            serde_json::json!("0"),
            serde_json::json!("-1"),
            serde_json::json!("18446744073709551616"),
            serde_json::Value::Null,
        ] {
            for key in ["dev.pichi.scute.verity.data-blocks", "dev.pichi.scute.size"] {
                let mut json = json.clone();
                json[key] = invalid.clone();
                assert!(serde_json::from_value::<ScuteAnnotations>(json).is_err());
            }
        }
    }

    #[test]
    fn content_identities_are_required_string_annotations() {
        let legacy = serde_json::json!({
            "dev.pichi.scute.verity.data-blocks": "2",
            "dev.pichi.scute.verity.salt": "00".repeat(32),
        });
        assert!(serde_json::from_value::<ScuteAnnotations>(legacy.clone()).is_err());
        let mut current = legacy;
        current["dev.pichi.scute.digest"] = format!("sha256:{}", "12".repeat(32)).into();
        current["dev.pichi.scute.size"] = "8192".into();
        current["dev.pichi.scute.verity.digest"] = format!("sha256:{}", "34".repeat(32)).into();
        let annotations: ScuteAnnotations = serde_json::from_value(current.clone()).unwrap();
        assert_eq!(serde_json::to_value(annotations).unwrap(), current);
        for key in [
            "dev.pichi.scute.digest",
            "dev.pichi.scute.size",
            "dev.pichi.scute.verity.digest",
        ] {
            let mut missing = current.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(serde_json::from_value::<ScuteAnnotations>(missing).is_err());
            for value in [serde_json::json!(8192), serde_json::Value::Null] {
                let mut invalid = current.clone();
                invalid[key] = value;
                assert!(serde_json::from_value::<ScuteAnnotations>(invalid).is_err());
            }
        }
    }

    #[test]
    fn validate_content_identities_against_format_and_geometry() {
        let mut manifest = sample_base_manifest();
        let Layer::Scute(scute) = &mut manifest.layers[0] else {
            unreachable!()
        };
        scute.annotations.digest = scute.digest.clone();
        scute.annotations.size = NonZeroU64::new(scute.size).unwrap().into();
        scute.annotations.verity_digest = Digest::Sha256([0x34; 32]);
        manifest.validate().unwrap();
        let original = manifest.layers[0].clone();
        let Layer::Scute(scute) = &mut manifest.layers[0] else {
            unreachable!()
        };
        scute.annotations.digest = Digest::Sha256([0xff; 32]);
        assert!(matches!(
            manifest.validate(),
            Err(ManifestValidationError::BadScuteAnnotation {
                key: "dev.pichi.scute.digest",
                ..
            })
        ));
        manifest.layers[0] = original.clone();
        for size in [1023, 1025] {
            let Layer::Scute(scute) = &mut manifest.layers[0] else {
                unreachable!()
            };
            scute.annotations.size = NonZeroU64::new(size).unwrap().into();
            assert!(manifest.validate().is_err());
        }
        let Layer::Scute(mut compressed) = original else {
            unreachable!()
        };
        compressed.size = 100;
        // Existing format permits a partial final block in inherited scutes.
        compressed.annotations.size = NonZeroU64::new(1023).unwrap().into();
        manifest.layers[0] = Layer::Scute(crate::artifact::ScuteDescriptor {
            media_type: crate::artifact::Compressable::Zstd(Default::default()),
            ..compressed
        });
        manifest.validate().unwrap();
    }

    #[test]
    fn validate_data_blocks_against_raw_descriptor() {
        for blocks in [1u64, 2, 1024, u64::MAX] {
            let mut manifest = sample_base_manifest();
            let Layer::Scute(scute) = &mut manifest.layers[0] else {
                unreachable!()
            };
            scute.annotations.data_blocks = std::num::NonZeroU64::new(blocks).unwrap().into();
            assert_eq!(manifest.validate().is_ok(), blocks == 1);
        }
        let mut manifest = sample_base_manifest();
        let Layer::Scute(mut scute) = manifest.layers.remove(0) else {
            unreachable!()
        };
        scute.annotations.data_blocks = std::num::NonZeroU64::new(2).unwrap().into();
        scute.annotations.size = NonZeroU64::new(8192).unwrap().into();
        manifest
            .layers
            .push(Layer::Scute(crate::artifact::ScuteDescriptor {
                media_type: crate::artifact::Compressable::Zstd(Default::default()),
                ..scute
            }));
        assert!(manifest.validate().is_ok());
        let Layer::Scute(scute) = &mut manifest.layers[0] else {
            unreachable!()
        };
        scute.annotations.data_blocks = std::num::NonZeroU64::MAX.into();
        assert!(matches!(
            manifest.validate(),
            Err(ManifestValidationError::BadScuteBlocks(_))
        ));
    }

    fn sample_base_manifest() -> Manifest {
        Manifest {
            schema_version: Version,
            media_type: Default::default(),
            artifact_type: Default::default(),
            config: ConfigDescriptor::for_config(Digest::Sha256([0x55; 32]), 42),
            layers: vec![Layer::Scute(ScuteDescriptor {
                media_type: crate::artifact::Compressable::Raw(Default::default()),
                digest: Digest::Sha256([0x11; 32]),
                size: 1024,
                annotations: ScuteAnnotations {
                    digest: Digest::Sha256([0x11; 32]),
                    size: NonZeroU64::new(1024).unwrap().into(),
                    verity_digest: Digest::Sha256([0x22; 32]),
                    data_blocks: std::num::NonZeroU64::new(1).unwrap().into(),
                    salt: Salt::new(&[0; 32]).unwrap().into(),
                },
            })],
            annotations: ManifestAnnotations {
                oci: Default::default(),
                build: super::super::BuildProvenance {
                    commit: Digest::Sha1([0xab; 20]),
                    context: ".".into(),
                    plan: "plan.yaml".into(),
                    lock: None,
                    location: None,
                    bootstrapped: false.into(),
                },
                verity: super::super::CarapaceVerity {
                    root: Some(Digest::Sha256([0xab; 32])),
                    ..Default::default()
                },
            },
        }
    }

    #[test]
    fn round_trip() {
        let m = sample_base_manifest();
        let b = serde_json::to_vec(&m).unwrap();
        let m2 = serde_json::from_slice::<Manifest>(&b).unwrap();
        m2.validate().unwrap();
        assert_eq!(m, m2);
    }

    #[test]
    fn round_trip_byte_stable() {
        let m = sample_base_manifest();
        let b1 = serde_json::to_vec(&m).unwrap();
        let m2 = serde_json::from_slice::<Manifest>(&b1).unwrap();
        let b2 = serde_json::to_vec(&m2).unwrap();
        assert_eq!(b1, b2, "manifest bytes must be stable across round-trip");
    }

    #[test]
    fn validate_accepts_base_manifest() {
        sample_base_manifest().validate().unwrap();
    }

    #[test]
    fn validate_accepts_bootable_image_with_one_pmi() {
        let mut m = sample_base_manifest();
        m.layers.push(Layer::Pmi(PmiDescriptor {
            media_type: crate::artifact::Compressable::Raw(Default::default()),
            digest: Digest::Sha256([0x22; 32]),
            size: 4096,
        }));
        m.validate().unwrap();
    }

    #[test]
    fn deserialization_requires_the_pichi_artifact_type_string() {
        let value = serde_json::to_value(sample_base_manifest()).unwrap();
        for invalid in [
            serde_json::json!("application/wrong"),
            serde_json::json!(null),
            serde_json::json!({PichiArtifact::MEDIA_TYPE: null}),
        ] {
            let mut value = value.clone();
            value["artifactType"] = invalid;
            assert!(serde_json::from_value::<Manifest>(value).is_err());
        }
    }

    #[test]
    fn config_media_type_is_required_and_fixed() {
        let value =
            serde_json::to_value(ConfigDescriptor::for_config(Digest::Sha256([0x55; 32]), 42))
                .unwrap();
        for invalid in [
            serde_json::json!("application/wrong"),
            serde_json::json!("application/vnd.oci.empty.v1+json"),
            serde_json::json!(null),
            serde_json::json!(1),
        ] {
            let mut value = value.clone();
            value["mediaType"] = invalid;
            assert!(serde_json::from_value::<ConfigDescriptor>(value).is_err());
        }
        let mut value = value;
        value.as_object_mut().unwrap().remove("mediaType");
        assert!(serde_json::from_value::<ConfigDescriptor>(value).is_err());
    }

    #[test]
    fn validate_accepts_real_config_blob() {
        let mut m = sample_base_manifest();
        m.config = ConfigDescriptor::for_config(Digest::Sha256([0x55; 32]), 42);
        m.validate().unwrap();
    }

    #[test]
    fn validate_accepts_omitted_and_partial_verity_defaults() {
        let mut m = sample_base_manifest();
        m.validate().unwrap();
        assert_eq!(m.carapace_root().unwrap(), Some([0xab; 32]));
        m.annotations.verity.data_block_size = "2048"
            .parse::<devmap_core::BlockSize<devmap_verity::Constraint>>()
            .unwrap()
            .into();
        let Layer::Scute(scute) = &mut m.layers[0] else {
            unreachable!()
        };
        scute.annotations.data_blocks = std::num::NonZeroU64::new(scute.size.div_ceil(2048))
            .unwrap()
            .into();
        m.validate().unwrap();
    }

    #[test]
    fn validate_rejects_multiple_pmi_layers() {
        let mut m = sample_base_manifest();
        for _ in 0..2 {
            m.layers.push(Layer::Pmi(PmiDescriptor {
                media_type: crate::artifact::Compressable::Raw(Default::default()),
                digest: Digest::Sha256([0x00; 32]),
                size: 1,
            }));
        }
        assert!(matches!(
            m.validate().unwrap_err(),
            ManifestValidationError::MultiplePmi(2)
        ));
    }

    #[test]
    fn deserialization_checks_salt_encoding() {
        let mut json = serde_json::to_value(sample_base_manifest()).unwrap();
        for salt in ["ZZZZ".to_owned(), "00".repeat(257)] {
            json["layers"][0]["annotations"]["dev.pichi.scute.verity.salt"] = salt.into();
            assert!(serde_json::from_value::<Manifest>(json.clone()).is_err());
        }
    }

    #[test]
    fn scutes_start_with_one_base_even_among_firmware_layers() {
        let mut manifest = sample_base_manifest();
        let Layer::Scute(mut child) = manifest.layers[0].clone() else {
            unreachable!()
        };
        // Only the parent prefix is the sentinel; the rest is ordinary salt.
        let Layer::Scute(base) = &mut manifest.layers[0] else {
            unreachable!()
        };
        base.annotations.salt = Salt::new(&[vec![0; 32], vec![0xab, 0xcd]].concat())
            .unwrap()
            .into();
        child.annotations.salt = Salt::new(&[0xab; 32]).unwrap().into();
        manifest.layers.insert(
            0,
            Layer::Pmi(PmiDescriptor {
                media_type: crate::artifact::Compressable::Raw(Default::default()),
                digest: Digest::Sha256([0x44; 32]),
                size: 8192,
            }),
        );
        manifest
            .layers
            .push(Layer::Scute(crate::artifact::ScuteDescriptor {
                media_type: crate::artifact::Compressable::Zstd(Default::default()),
                ..child
            }));
        manifest.validate().unwrap();

        manifest.layers.swap(1, 2);
        assert!(matches!(
            manifest.validate(),
            Err(ManifestValidationError::BadScuteOrder(_))
        ));
        manifest.layers.swap(1, 2);
        manifest.layers.push(manifest.layers[1].clone());
        assert!(matches!(
            manifest.validate(),
            Err(ManifestValidationError::BadScuteOrder(_))
        ));
    }

    #[test]
    fn scute_salts_must_contain_a_complete_parent_root() {
        for salt in [String::new(), "00".repeat(31)] {
            let mut manifest = sample_base_manifest();
            let Layer::Scute(base) = &mut manifest.layers[0] else {
                unreachable!()
            };
            base.annotations.salt = salt.parse::<Salt>().unwrap().into();
            assert!(matches!(
                manifest.validate(),
                Err(ManifestValidationError::BadSalt(_))
            ));
        }
    }

    #[test]
    fn validate_requires_one_nonzero_chain_tip() {
        let mut m = sample_base_manifest();
        assert_eq!(m.carapace_root().unwrap(), Some([0xab; 32]));
        m.annotations.verity.root = None;
        assert!(matches!(
            m.validate(),
            Err(ManifestValidationError::MissingChainAnnotation(_))
        ));
        m.annotations.verity.root = Some(Digest::Sha256([0; 32]));
        assert!(matches!(
            m.validate(),
            Err(ManifestValidationError::BadChainAnnotation { .. })
        ));
        m.annotations.verity.root = Some(Digest::Sha256([0xab; 32]));
        m.validate().unwrap();
        m.layers.clear();
        assert!(m.validate().is_err());
        m.annotations.verity = Default::default();
        m.validate().unwrap();
        assert_eq!(m.carapace_root().unwrap(), None);
    }

    #[test]
    fn validate_accepts_scuteless_manifest_without_chain_annotations() {
        // A PMI-only (scute-less) manifest has no carapace, so chain
        // verity annotations are not required.
        let mut m = sample_base_manifest();
        m.layers.clear();
        m.layers.push(Layer::Pmi(PmiDescriptor {
            media_type: crate::artifact::Compressable::Raw(Default::default()),
            digest: Digest::Sha256([0x22; 32]),
            size: 4096,
        }));
        m.annotations.verity = Default::default();
        m.validate().unwrap();
    }

    #[test]
    fn deserialize_unknown_layer_media_type_errors() {
        let value = serde_json::to_value(&sample_base_manifest().layers[0]).unwrap();
        for invalid in [
            serde_json::json!("application/unknown"),
            serde_json::json!(null),
            serde_json::json!(1),
        ] {
            let mut value = value.clone();
            value["mediaType"] = invalid;
            assert!(serde_json::from_value::<Layer>(value).is_err());
        }
        let mut value = value;
        value.as_object_mut().unwrap().remove("mediaType");
        assert!(serde_json::from_value::<Layer>(value).is_err());
    }

    #[test]
    fn media_type_constant_used() {
        // Exercises PichiArtifact::MEDIA_TYPE to satisfy dead_code lint.
        assert!(PichiArtifact::MEDIA_TYPE.starts_with("application/"));
    }

    #[test]
    fn pichi_layer_is_zstd_variant() {
        let scute_desc = ScuteDescriptor {
            media_type: crate::artifact::Compressable::Raw(Default::default()),
            digest: Digest::Sha256([0xde; 32]),
            size: 0,
            annotations: ScuteAnnotations {
                digest: Digest::Sha256([0x11; 32]),
                size: NonZeroU64::new(1024).unwrap().into(),
                verity_digest: Digest::Sha256([0x22; 32]),
                data_blocks: std::num::NonZeroU64::new(1).unwrap().into(),
                salt: Salt::new(&[0xde, 0xad, 0xbe, 0xef]).unwrap().into(),
            },
        };
        let pmi_desc = PmiDescriptor {
            media_type: crate::artifact::Compressable::Raw(Default::default()),
            digest: Digest::Sha256([0xbe; 32]),
            size: 0,
        };
        assert!(
            !Layer::Scute(crate::artifact::ScuteDescriptor {
                media_type: crate::artifact::Compressable::Raw(Default::default()),
                ..scute_desc.clone()
            })
            .is_zstd_variant()
        );
        assert!(
            Layer::Scute(crate::artifact::ScuteDescriptor {
                media_type: crate::artifact::Compressable::Zstd(Default::default()),
                ..scute_desc
            })
            .is_zstd_variant()
        );
        assert!(
            !Layer::Pmi(crate::artifact::PmiDescriptor {
                media_type: crate::artifact::Compressable::Raw(Default::default()),
                ..pmi_desc
            })
            .is_zstd_variant()
        );
    }

    #[test]
    fn pichi_layer_media_type_str() {
        let scute_desc = ScuteDescriptor {
            media_type: crate::artifact::Compressable::Raw(Default::default()),
            digest: Digest::Sha256([0xde; 32]),
            size: 0,
            annotations: ScuteAnnotations {
                digest: Digest::Sha256([0x11; 32]),
                size: NonZeroU64::new(1024).unwrap().into(),
                verity_digest: Digest::Sha256([0x22; 32]),
                data_blocks: std::num::NonZeroU64::new(1).unwrap().into(),
                salt: Salt::new(&[0xde, 0xad, 0xbe, 0xef]).unwrap().into(),
            },
        };
        let pmi_desc = PmiDescriptor {
            media_type: crate::artifact::Compressable::Raw(Default::default()),
            digest: Digest::Sha256([0xbe; 32]),
            size: 0,
        };
        assert_eq!(
            Layer::Scute(crate::artifact::ScuteDescriptor {
                media_type: crate::artifact::Compressable::Raw(Default::default()),
                ..scute_desc.clone()
            })
            .media_type_str(),
            ScuteDescriptor::MEDIA_TYPE
        );
        assert_eq!(
            Layer::Scute(crate::artifact::ScuteDescriptor {
                media_type: crate::artifact::Compressable::Zstd(Default::default()),
                ..scute_desc
            })
            .media_type_str(),
            crate::artifact::Zstd::<crate::artifact::ScuteDescriptor>::MEDIA_TYPE
        );
        assert_eq!(
            Layer::Pmi(crate::artifact::PmiDescriptor {
                media_type: crate::artifact::Compressable::Raw(Default::default()),
                ..pmi_desc
            })
            .media_type_str(),
            PmiDescriptor::MEDIA_TYPE
        );
    }
}
