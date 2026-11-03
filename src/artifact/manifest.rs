// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Pichi artifact structure and component validation.

use super::ScuteAnnotations;
use crate::resources::MEDIA_TYPE_PICHI_CONFIG_V1;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::media_type::{
    MEDIA_TYPE_PICHI_ARTIFACT_V1, MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD, MEDIA_TYPE_PICHI_PMI_V1,
    MEDIA_TYPE_PICHI_SCUTE_V1, MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
};

/// Required chain-wide annotation keys (per D-06). These are the dm-verity
/// superblock fields a consumer needs to *reconstruct* the (undistributed)
/// verity layer from the cow — the sole reason pichi carries them. All MUST be
/// present in `Manifest::annotations` per D-07 rule 3 when scute layers exist.
const CHAIN_ANNOTATION_VERITY_ALGO: &str = "dev.pichi.carapace.verity.algo";
const CHAIN_ANNOTATION_VERITY_DATA_BLOCK_SIZE: &str = "dev.pichi.carapace.verity.data-block-size";
const CHAIN_ANNOTATION_VERITY_HASH_BLOCK_SIZE: &str = "dev.pichi.carapace.verity.hash-block-size";
/// dm-verity superblock `version` and `hash_type` — reconstruction inputs that
/// were previously hardcoded in the builder rather than conveyed.
const CHAIN_ANNOTATION_VERITY_VERSION: &str = "dev.pichi.carapace.verity.version";
const CHAIN_ANNOTATION_VERITY_HASH_TYPE: &str = "dev.pichi.carapace.verity.hash-type";

/// Hex-encoded SHA-256 root of the final scute, committing the salt-linked chain.
pub const CARAPACE_ROOT_ANNOTATION: &str = "dev.pichi.carapace.verity.hash";

/// Canonical SHA-256 digest of `{}` (the OCI 1.1 empty config blob).
const EMPTY_CONFIG_DIGEST: &str =
    "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
/// Canonical base64 encoding of `{}` (used as inline `data` per OCI 1.1).
const EMPTY_CONFIG_DATA_BASE64: &str = "e30=";

/// pichi OCI artifact manifest (OCI Image Spec 1.1).
///
/// Schema invariants (per CONTEXT.md D-03 / D-07 — enforced by [`Manifest::validate`]):
/// - Top-level `artifact_type` MUST equal [`MEDIA_TYPE_PICHI_ARTIFACT_V1`].
/// - `config` MUST be the OCI 1.1 empty-config descriptor with inline `data: "e30="`.
/// - Scute layers MUST appear in chain order, from base to tip.
/// - At most ONE PMI component (D-03 — zero pmi = base/non-bootable, one pmi = appliance/bootable).
/// - Top-level annotations MUST include the carapace verity *param* keys
///   and one chain-tip root when scute layers exist.
/// - Every scute layer MUST carry a `dev.pichi.scute.verity.salt` annotation
///   parseable as hex (D-05 / D-07 rule 4-5).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    /// Always `2` for OCI 1.1.
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    /// OCI image manifest media type (`application/vnd.oci.image.manifest.v1+json`).
    /// Carried as `String` (NOT `Option`) per D-03 — the pichi manifest is always
    /// the OCI 1.1 image manifest envelope.
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// OCI 1.1 wrapper type. MUST be `MEDIA_TYPE_PICHI_ARTIFACT_V1` per D-07 rule 1.
    #[serde(rename = "artifactType")]
    pub artifact_type: String,
    /// Manifest config descriptor: either the OCI 1.1 empty config (artifacts
    /// with no launch contract) or a real `vnd.pichi.config.v1+json` blob
    /// carrying the launch contract (BUILD.md §7.1).
    pub config: oci_spec::image::Descriptor,
    /// Typed layer descriptors. Scutes appear in base-to-tip chain order;
    /// PMI and boot configuration descriptors are outside that order.
    pub layers: Vec<oci_spec::image::Descriptor>,
    /// Top-level annotations. With scutes, MUST include
    /// `dev.pichi.carapace.verity.{algo,data-block-size,hash-block-size,
    /// version,hash-type,hash}` and SHOULD include
    /// `org.opencontainers.image.created` (D-06).
    /// `BTreeMap` for byte-stable JSON serialisation (lexicographic key order).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
}

/// Validation errors for [`Manifest::validate`] (per D-07).
#[derive(Debug, Error)]
pub enum ManifestValidationError {
    /// The artifact contains a component Pichi does not support.
    #[error("unsupported artifact layer media type: {0}")]
    UnsupportedLayer(String),

    /// D-07 rule 1: wrong `artifactType`.
    #[error("artifactType must be {expected}, got {actual}")]
    WrongArtifactType {
        /// Expected value.
        expected: String,
        /// Actual value found.
        actual: String,
    },
    /// D-07 rule 2: config descriptor is neither the canonical OCI empty config
    /// nor a `vnd.pichi.config.v1+json` blob.
    #[error(
        "config descriptor mediaType {media_type} is not the OCI empty config or {MEDIA_TYPE_PICHI_CONFIG_V1}"
    )]
    BadConfig {
        /// The offending config mediaType.
        media_type: String,
    },
    /// D-07 rule 3: required chain annotation key is absent.
    #[error("missing required chain annotation: {0}")]
    MissingChainAnnotation(&'static str),
    /// A chain annotation is present but malformed (e.g. non-integer
    /// version/hash-type). This is a well-formedness check only — pichi imposes
    /// no value whitelist (that is the guest consumer's control per the spec).
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

    /// Scute salt is not valid hex or lacks the 32-byte parent root prefix.
    #[error("scute salt must contain a hex-encoded parent root: {0}")]
    BadSalt(String),

    /// The first scute must be the base; later scutes must have a parent.
    #[error("scutes must appear in base-to-tip order: {0}")]
    BadScuteOrder(String),

    /// Declared data-block count is zero or contradicts a raw blob's size.
    #[error("invalid data-block count for scute {0}")]
    BadScuteBlocks(String),
}

impl Manifest {
    /// The chain-tip commitment, present exactly when this artifact has scutes.
    /// This parses the wire value; consumers must authenticate the manifest and
    /// verify scute reads against this root before using their contents.
    pub fn carapace_root(&self) -> Result<Option<[u8; 32]>, ManifestValidationError> {
        let value = self.annotations.get(CARAPACE_ROOT_ANNOTATION);
        if !self.has_scutes() && value.is_none() {
            return Ok(None);
        }
        let value = value.ok_or(ManifestValidationError::MissingChainAnnotation(
            CARAPACE_ROOT_ANNOTATION,
        ))?;
        let mut root = [0; 32];
        if !self.has_scutes() || hex::decode_to_slice(value, &mut root).is_err() || root == [0; 32]
        {
            return Err(ManifestValidationError::BadChainAnnotation {
                key: CARAPACE_ROOT_ANNOTATION,
                value: value.clone(),
            });
        }
        Ok(Some(root))
    }

    /// Return `true` if this manifest carries at least one scute layer (i.e.
    /// it describes a carapace, so a verity root anchor is meaningful).
    fn has_scutes(&self) -> bool {
        self.layers.iter().any(|l| {
            matches!(
                l.media_type().as_ref(),
                crate::artifact::MEDIA_TYPE_PICHI_SCUTE_V1
                    | crate::artifact::MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD
            )
        })
    }

    /// Validate manifest structure. Scute order starts with a zero-parent base;
    /// consumers must verify each subsequent parent against the preceding
    /// scute's content and the final root against the manifest commitment.
    ///
    /// # Errors
    /// Returns the first violated rule — does NOT collect all errors.
    pub fn validate(&self) -> Result<(), ManifestValidationError> {
        // Rule 1: artifactType
        if self.artifact_type != MEDIA_TYPE_PICHI_ARTIFACT_V1 {
            return Err(ManifestValidationError::WrongArtifactType {
                expected: MEDIA_TYPE_PICHI_ARTIFACT_V1.to_string(),
                actual: self.artifact_type.clone(),
            });
        }
        // Rule 2: config is either the canonical empty config or a real
        // pichi config blob (vnd.pichi.config.v1+json).
        let c = &self.config;
        let is_canonical_empty = c.media_type() == &oci_spec::image::MediaType::EmptyJSON
            && c.size() == 2
            && c.data()
                .as_deref()
                .is_none_or(|data| data == EMPTY_CONFIG_DATA_BASE64)
            && c.digest().to_string() == EMPTY_CONFIG_DIGEST;
        let is_config_blob = c.media_type().as_ref() == MEDIA_TYPE_PICHI_CONFIG_V1;
        if !is_canonical_empty && !is_config_blob {
            return Err(ManifestValidationError::BadConfig {
                media_type: c.media_type().to_string(),
            });
        }
        // Rule 3 (BUILD.md §7.1): the carapace verity-chain annotations describe
        // the carapace, so they are required ONLY when scute layers are present.
        // A scute-less (PMI-only) artifact carries none of them.
        self.carapace_root()?;
        if self.has_scutes() {
            for key in [
                CHAIN_ANNOTATION_VERITY_ALGO,
                CHAIN_ANNOTATION_VERITY_DATA_BLOCK_SIZE,
                CHAIN_ANNOTATION_VERITY_HASH_BLOCK_SIZE,
                CHAIN_ANNOTATION_VERITY_VERSION,
                CHAIN_ANNOTATION_VERITY_HASH_TYPE,
            ] {
                if !self.annotations.contains_key(key) {
                    return Err(ManifestValidationError::MissingChainAnnotation(key));
                }
            }
            // Well-formedness only (no value whitelist — that is the guest
            // consumer's TCB control per the carapace spec, not storage's):
            // version / hash-type must parse as integers.
            for key in [
                CHAIN_ANNOTATION_VERITY_VERSION,
                CHAIN_ANNOTATION_VERITY_HASH_TYPE,
            ] {
                let v = &self.annotations[key];
                if v.parse::<u32>().is_err() {
                    return Err(ManifestValidationError::BadChainAnnotation {
                        key,
                        value: v.clone(),
                    });
                }
            }
        }
        // Rule 5 (out of order — bail early on count): at most one PMI
        let pmi_count = self
            .layers
            .iter()
            .filter(|l| {
                matches!(
                    l.media_type().as_ref(),
                    crate::artifact::MEDIA_TYPE_PICHI_PMI_V1
                )
            })
            .count();
        if pmi_count > 1 {
            return Err(ManifestValidationError::MultiplePmi(pmi_count));
        }
        let boot_config_count = self
            .layers
            .iter()
            .filter(|layer| {
                matches!(
                    layer.media_type().as_ref(),
                    crate::artifact::MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD
                )
            })
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
            match layer.media_type().as_ref() {
                MEDIA_TYPE_PICHI_PMI_V1 | MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD => continue,
                MEDIA_TYPE_PICHI_SCUTE_V1 | MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD => (),
                other => return Err(ManifestValidationError::UnsupportedLayer(other.into())),
            }
            let annotations = ScuteAnnotations::try_from(layer)?;
            let blocks = annotations.data_blocks.get();
            let value = &self.annotations[CHAIN_ANNOTATION_VERITY_DATA_BLOCK_SIZE];
            let block_size = value
                .parse::<u32>()
                .ok()
                .filter(|size| *size != 0)
                .ok_or_else(|| ManifestValidationError::BadChainAnnotation {
                    key: CHAIN_ANNOTATION_VERITY_DATA_BLOCK_SIZE,
                    value: value.clone(),
                })?;
            if blocks.checked_mul(u64::from(block_size)).is_none()
                || (matches!(
                    layer.media_type().as_ref(),
                    crate::artifact::MEDIA_TYPE_PICHI_SCUTE_V1
                ) && blocks != layer.size().div_ceil(u64::from(block_size)))
            {
                return Err(ManifestValidationError::BadScuteBlocks(
                    layer.digest().to_string(),
                ));
            }
            let salt = hex::decode(&annotations.salt)
                .map_err(|_| ManifestValidationError::BadSalt(annotations.salt.clone()))?;
            let parent = salt
                .get(..32)
                .ok_or_else(|| ManifestValidationError::BadSalt(annotations.salt.clone()))?;
            if first_scute != parent.iter().all(|byte| *byte == 0) {
                return Err(ManifestValidationError::BadScuteOrder(
                    layer.digest().to_string(),
                ));
            }
            first_scute = false;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest as _, Sha256};

    #[test]
    fn boot_config_is_a_compressed_component_requiring_one_pmi() {
        let config = oci_spec::image::Descriptor::new(
            crate::artifact::MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD.into(),
            73u64,
            format!("sha256:{}", "34".repeat(32))
                .parse::<oci_spec::image::Digest>()
                .expect("layer digest"),
        );
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(
            json["mediaType"],
            "application/vnd.pichi.bootconfig.v1+zstd"
        );
        assert_eq!(json["size"], 73);
        assert_eq!(
            serde_json::from_value::<oci_spec::image::Descriptor>(json).unwrap(),
            config
        );
        assert_eq!(
            config.media_type().as_ref(),
            MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD
        );

        for root in [false, true] {
            let mut manifest = sample_base_manifest();
            if !root {
                manifest.layers.clear();
                manifest.annotations.clear();
            }
            manifest.layers.push(config.clone());
            assert!(matches!(
                manifest.validate(),
                Err(ManifestValidationError::BootConfigWithoutPmi)
            ));
            manifest.layers.push(oci_spec::image::Descriptor::new(
                crate::artifact::MEDIA_TYPE_PICHI_PMI_V1.into(),
                4096u64,
                format!("sha256:{}", "12".repeat(32))
                    .parse::<oci_spec::image::Digest>()
                    .expect("layer digest"),
            ));
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
        let mut descriptor = sample_base_manifest().layers.remove(0);
        let annotations = ScuteAnnotations {
            salt: "00".into(),
            data_blocks: 2u64.try_into().unwrap(),
        };
        descriptor.set_annotations(Some(annotations.clone().into()));
        let json = serde_json::to_value(&descriptor).unwrap();
        assert_eq!(
            json["annotations"]["dev.pichi.scute.verity.data-blocks"],
            "2"
        );
        assert_eq!(
            ScuteAnnotations::try_from(&descriptor).unwrap(),
            annotations
        );
        descriptor.set_annotations(None);
        assert!(ScuteAnnotations::try_from(&descriptor).is_err());
        for invalid in [
            serde_json::json!(4097),
            serde_json::json!(""),
            serde_json::json!("0"),
            serde_json::json!("-1"),
            serde_json::json!("+1"),
            serde_json::json!("18446744073709551616"),
            serde_json::Value::Null,
        ] {
            let mut json = json.clone();
            json["annotations"]["dev.pichi.scute.verity.data-blocks"] = invalid;
            assert!(
                serde_json::from_value::<oci_spec::image::Descriptor>(json)
                    .ok()
                    .and_then(|descriptor| ScuteAnnotations::try_from(&descriptor).ok())
                    .is_none()
            );
        }
    }

    #[test]
    fn validate_data_blocks_against_raw_descriptor() {
        for blocks in [1u64, 2, 1024, u64::MAX] {
            let mut manifest = sample_base_manifest();
            let scute = &mut manifest.layers[0];
            let mut annotations = ScuteAnnotations::try_from(&*scute).unwrap();
            annotations.data_blocks = blocks.try_into().unwrap();
            scute.set_annotations(Some(annotations.into()));
            assert_eq!(manifest.validate().is_ok(), blocks == 1);
        }
        let mut manifest = sample_base_manifest();
        let mut scute = manifest.layers.remove(0);
        let mut annotations = ScuteAnnotations::try_from(&scute).unwrap();
        annotations.data_blocks = 2u64.try_into().unwrap();
        scute.set_annotations(Some(annotations.into()));
        manifest.layers.push({
            let mut descriptor = scute;
            descriptor.set_media_type(crate::artifact::MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD.into());
            descriptor
        });
        assert!(manifest.validate().is_ok());
        let scute = &mut manifest.layers[0];
        let mut annotations = ScuteAnnotations::try_from(&*scute).unwrap();
        annotations.data_blocks = u64::MAX.try_into().unwrap();
        scute.set_annotations(Some(annotations.into()));
        assert!(matches!(
            manifest.validate(),
            Err(ManifestValidationError::BadScuteBlocks(_))
        ));
    }

    fn sample_chain_annotations() -> BTreeMap<String, String> {
        let mut a = BTreeMap::new();
        a.insert(CHAIN_ANNOTATION_VERITY_ALGO.into(), "sha256".into());
        a.insert(
            CHAIN_ANNOTATION_VERITY_DATA_BLOCK_SIZE.into(),
            "4096".into(),
        );
        a.insert(
            CHAIN_ANNOTATION_VERITY_HASH_BLOCK_SIZE.into(),
            "4096".into(),
        );
        a.insert(CHAIN_ANNOTATION_VERITY_VERSION.into(), "1".into());
        a.insert(CHAIN_ANNOTATION_VERITY_HASH_TYPE.into(), "1".into());
        a.insert(CARAPACE_ROOT_ANNOTATION.into(), "ab".repeat(32));
        a.insert(
            "org.opencontainers.image.created".into(),
            "2026-05-06T14:32:00Z".into(),
        );
        a
    }

    fn sample_base_manifest() -> Manifest {
        Manifest {
            schema_version: 2,
            media_type: "application/vnd.oci.image.manifest.v1+json".into(),
            artifact_type: MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
            config: oci_spec::image::DescriptorBuilder::default()
                .media_type(oci_spec::image::MediaType::EmptyJSON)
                .digest(
                    "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
                        .parse::<oci_spec::image::Digest>()
                        .expect("empty config digest"),
                )
                .size(2u64)
                .data("e30=")
                .build()
                .expect("empty config descriptor"),
            layers: vec![{
                let mut descriptor = oci_spec::image::Descriptor::new(
                    crate::artifact::MEDIA_TYPE_PICHI_SCUTE_V1.into(),
                    1024u64,
                    "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                        .parse::<oci_spec::image::Digest>()
                        .expect("layer digest"),
                );
                descriptor.set_annotations(Some(
                    std::collections::HashMap::<String, String>::from(ScuteAnnotations {
                        data_blocks: 1u64.try_into().unwrap(),
                        salt: "00".repeat(32),
                    }),
                ));
                descriptor
            }],
            annotations: sample_chain_annotations(),
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
    fn round_trip_preserves_additional_layer_annotations() {
        let mut manifest = sample_base_manifest();
        let mut annotations = manifest.layers[0].annotations().clone().unwrap();
        annotations.insert("org.example.description".into(), "retained".into());
        manifest.layers[0].set_annotations(Some(annotations));
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let decoded: Manifest = serde_json::from_slice(&bytes).unwrap();
        decoded.validate().unwrap();
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn empty_config_canonical_matches_oci_spec() {
        let c = oci_spec::image::DescriptorBuilder::default()
            .media_type(oci_spec::image::MediaType::EmptyJSON)
            .digest(
                "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"
                    .parse::<oci_spec::image::Digest>()
                    .expect("empty config digest"),
            )
            .size(2u64)
            .data("e30=")
            .build()
            .expect("empty config descriptor");
        assert_eq!(c.media_type(), &oci_spec::image::MediaType::EmptyJSON);
        assert_eq!(c.size(), 2);
        assert_eq!(c.data().as_deref(), Some("e30="));
        assert_eq!(c.digest().to_string(), EMPTY_CONFIG_DIGEST);
        // Cross-check: SHA-256 of "{}" really is the EMPTY_CONFIG_DIGEST hex
        let manual = format!("sha256:{}", hex::encode(Sha256::digest(b"{}")))
            .parse::<oci_spec::image::Digest>()
            .expect("SHA-256 digest")
            .to_string();
        assert_eq!(manual, EMPTY_CONFIG_DIGEST);
    }

    #[test]
    fn empty_config_data_is_optional() {
        let mut manifest = sample_base_manifest();
        manifest.config.set_data(None);
        manifest.validate().unwrap();
    }

    #[test]
    fn validate_accepts_base_manifest() {
        sample_base_manifest().validate().unwrap();
    }

    #[test]
    fn validate_accepts_appliance_with_one_pmi() {
        let mut m = sample_base_manifest();
        m.layers.push(oci_spec::image::Descriptor::new(
            crate::artifact::MEDIA_TYPE_PICHI_PMI_V1.into(),
            4096u64,
            "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .parse::<oci_spec::image::Digest>()
                .expect("layer digest"),
        ));
        m.validate().unwrap();
    }

    #[test]
    fn validate_rejects_wrong_artifact_type() {
        let mut m = sample_base_manifest();
        m.artifact_type = "application/wrong".into();
        let err = m.validate().unwrap_err();
        assert!(matches!(
            err,
            ManifestValidationError::WrongArtifactType { .. }
        ));
    }

    #[test]
    fn validate_rejects_bad_config() {
        let mut m = sample_base_manifest();
        m.config
            .set_media_type(oci_spec::image::MediaType::ImageConfig);
        assert!(matches!(
            m.validate().unwrap_err(),
            ManifestValidationError::BadConfig { .. }
        ));
    }

    #[test]
    fn validate_accepts_real_config_blob() {
        let mut m = sample_base_manifest();
        m.config = oci_spec::image::Descriptor::new(
            crate::resources::MEDIA_TYPE_PICHI_CONFIG_V1.into(),
            42,
            ("sha256:5555555555555555555555555555555555555555555555555555555555555555")
                .parse::<oci_spec::image::Digest>()
                .expect("config digest"),
        );
        m.validate().unwrap();
    }

    #[test]
    fn validate_rejects_missing_chain_annotation() {
        let mut m = sample_base_manifest();
        m.annotations.remove(CHAIN_ANNOTATION_VERITY_ALGO);
        let err = m.validate().unwrap_err();
        assert!(
            matches!(err, ManifestValidationError::MissingChainAnnotation(k) if k == CHAIN_ANNOTATION_VERITY_ALGO)
        );
    }

    #[test]
    fn validate_rejects_multiple_pmi_layers() {
        let mut m = sample_base_manifest();
        for _ in 0..2 {
            m.layers.push(oci_spec::image::Descriptor::new(
                crate::artifact::MEDIA_TYPE_PICHI_PMI_V1.into(),
                1u64,
                "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .parse::<oci_spec::image::Digest>()
                    .expect("layer digest"),
            ));
        }
        assert!(matches!(
            m.validate().unwrap_err(),
            ManifestValidationError::MultiplePmi(2)
        ));
    }

    #[test]
    fn validate_rejects_non_hex_salt() {
        let mut m = sample_base_manifest();
        let mut annotations = ScuteAnnotations::try_from(&m.layers[0]).unwrap();
        annotations.salt = "ZZZZ".into();
        m.layers[0].set_annotations(Some(annotations.into()));
        assert!(matches!(
            m.validate().unwrap_err(),
            ManifestValidationError::BadSalt(_)
        ));
    }

    #[test]
    fn scutes_start_with_one_base_even_among_firmware_layers() {
        let mut manifest = sample_base_manifest();
        let mut child = manifest.layers[0].clone();
        // Only the parent prefix is the sentinel; the rest is ordinary salt.
        let base = &mut manifest.layers[0];
        let mut annotations = ScuteAnnotations::try_from(&*base).unwrap();
        annotations.salt.push_str("abcd");
        base.set_annotations(Some(annotations.into()));
        let mut annotations = ScuteAnnotations::try_from(&child).unwrap();
        annotations.salt = "ab".repeat(32);
        child.set_annotations(Some(annotations.into()));
        manifest.layers.insert(
            0,
            oci_spec::image::Descriptor::new(
                crate::artifact::MEDIA_TYPE_PICHI_PMI_V1.into(),
                8192u64,
                format!("sha256:{}", "44".repeat(32))
                    .parse::<oci_spec::image::Digest>()
                    .expect("layer digest"),
            ),
        );
        manifest.layers.push({
            let mut descriptor = child;
            descriptor.set_media_type(crate::artifact::MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD.into());
            descriptor
        });
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
            let base = &mut manifest.layers[0];
            let mut annotations = ScuteAnnotations::try_from(&*base).unwrap();
            annotations.salt = salt;
            base.set_annotations(Some(annotations.into()));
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
        m.annotations.remove(CARAPACE_ROOT_ANNOTATION);
        assert!(matches!(
            m.validate(),
            Err(ManifestValidationError::MissingChainAnnotation(
                CARAPACE_ROOT_ANNOTATION
            ))
        ));
        for invalid in [
            "".into(),
            "deadbeef".into(),
            "gg".repeat(32),
            "00".repeat(32),
            "ab".repeat(33),
            format!("sha256:{}", "ab".repeat(32)),
        ] {
            m.annotations
                .insert(CARAPACE_ROOT_ANNOTATION.into(), invalid);
            assert!(matches!(
                m.validate(),
                Err(ManifestValidationError::BadChainAnnotation {
                    key: CARAPACE_ROOT_ANNOTATION,
                    ..
                })
            ));
        }
        m.annotations
            .insert(CARAPACE_ROOT_ANNOTATION.into(), "AB".repeat(32));
        m.validate().unwrap();
        m.layers.clear();
        assert!(m.validate().is_err());
        m.annotations.clear();
        m.validate().unwrap();
        assert_eq!(m.carapace_root().unwrap(), None);
    }

    #[test]
    fn validate_requires_verity_version_and_hash_type_when_scutes_present() {
        for key in [
            CHAIN_ANNOTATION_VERITY_VERSION,
            CHAIN_ANNOTATION_VERITY_HASH_TYPE,
        ] {
            let mut m = sample_base_manifest();
            m.annotations.remove(key);
            assert!(
                matches!(
                    m.validate().unwrap_err(),
                    ManifestValidationError::MissingChainAnnotation(k) if k == key
                ),
                "missing {key} must be rejected"
            );
        }
    }

    #[test]
    fn validate_rejects_malformed_verity_version() {
        // Well-formedness only: non-integer version is rejected (but any integer
        // is accepted — pichi imposes no value whitelist).
        let mut m = sample_base_manifest();
        m.annotations.insert(
            CHAIN_ANNOTATION_VERITY_VERSION.into(),
            "not-a-number".into(),
        );
        assert!(matches!(
            m.validate().unwrap_err(),
            ManifestValidationError::BadChainAnnotation { key, .. }
                if key == CHAIN_ANNOTATION_VERITY_VERSION
        ));
    }

    #[test]
    fn validate_accepts_scuteless_manifest_without_chain_annotations() {
        // A PMI-only (scute-less) manifest has no carapace, so chain
        // verity annotations are not required.
        let mut m = sample_base_manifest();
        m.layers.clear();
        m.layers.push(oci_spec::image::Descriptor::new(
            crate::artifact::MEDIA_TYPE_PICHI_PMI_V1.into(),
            4096u64,
            "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .parse::<oci_spec::image::Digest>()
                .expect("layer digest"),
        ));
        m.annotations.clear();
        m.validate().unwrap();
    }

    #[test]
    fn validation_rejects_unsupported_components_after_oci_decoding() {
        let mut manifest = sample_base_manifest();
        let mut unknown = manifest.layers[0].clone();
        unknown.set_media_type("application/vnd.unknown.v1".into());
        manifest.layers.push(unknown);
        let decoded: Manifest =
            serde_json::from_value(serde_json::to_value(manifest).unwrap()).unwrap();
        assert!(matches!(
            decoded.validate(),
            Err(ManifestValidationError::UnsupportedLayer(_))
        ));
    }

    #[test]
    fn media_type_constant_used() {
        // Exercises MEDIA_TYPE_PICHI_ARTIFACT_V1 to satisfy dead_code lint.
        assert!(MEDIA_TYPE_PICHI_ARTIFACT_V1.starts_with("application/"));
    }
}
