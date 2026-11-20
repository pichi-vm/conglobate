// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! `Manifest` typed schema (Phase 42 FORMAT-01..03). Per D-03, manifests
//! are content-addressed JSON stored in the blob store; this type provides
//! parse / serialise / validate / self-digest helpers.
//!
//! Phase 42 locks the schema: typed `layers: Vec<Layer>` (internally-
//! tagged on `mediaType`), `ConfigDescriptor` for the OCI 1.1 empty-config
//! pattern, and `validate()` enforcing all six D-07 rules.

use super::{AsString, BootConfigDescriptor, Digest, ManifestAnnotations};
use crate::is_default::IsDefault;

use crate::plan::Version;
use devmap_verity::Salt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::media_type::{
    MEDIA_TYPE_OCI_EMPTY_V1, MEDIA_TYPE_PICHI_ARTIFACT_V1, MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD,
    MEDIA_TYPE_PICHI_CONFIG_V1, MEDIA_TYPE_PICHI_PMI_V1, MEDIA_TYPE_PICHI_PMI_V1_ZSTD,
    MEDIA_TYPE_PICHI_SCUTE_V1, MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
};

/// Algorithm-qualified SHA-256 root of the final scute.
pub const CARAPACE_ROOT_ANNOTATION: &str = "dev.pichi.carapace.verity.hash";

/// Canonical SHA-256 digest of `{}` (the OCI 1.1 empty config blob).
const EMPTY_CONFIG_DIGEST: Digest = Digest::Sha256([
    0x44, 0x13, 0x6f, 0xa3, 0x55, 0xb3, 0x67, 0x8a, 0x11, 0x46, 0xad, 0x16, 0xf7, 0xe8, 0x64, 0x9e,
    0x94, 0xfb, 0x4f, 0xc2, 0x1f, 0xe7, 0x7e, 0x83, 0x10, 0xc0, 0x60, 0xf6, 0x1c, 0xaa, 0xff, 0x8a,
]);
/// Canonical base64 encoding of `{}` (used as inline `data` per OCI 1.1).
const EMPTY_CONFIG_DATA_BASE64: &str = "e30=";

/// pichi OCI artifact manifest (OCI Image Spec 1.1).
///
/// Schema invariants (per CONTEXT.md D-03 / D-07 — enforced by [`Manifest::validate`]):
/// - Top-level `artifact_type` MUST equal [`MEDIA_TYPE_PICHI_ARTIFACT_V1`].
/// - `config` MUST be the OCI 1.1 empty-config descriptor with inline `data: "e30="`.
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
    pub config: ConfigDescriptor,
    /// Typed layer descriptors. Scutes appear in base-to-tip chain order;
    /// PMI and boot configuration descriptors are outside that order.
    pub layers: Vec<Layer>,
    /// Typed annotation view. Verity parameters have native defaults;
    /// unrelated annotations remain in the original manifest bytes.
    #[serde(default, skip_serializing_if = "IsDefault::is_default")]
    pub annotations: ManifestAnnotations,
}

/// The manifest `config` descriptor. Either the OCI 1.1 empty config (inline
/// `data: "e30="`, for artifacts with no launch contract) or a real
/// `application/vnd.pichi.config.v1+json` blob referenced by digest (BUILD.md
/// §7.1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDescriptor {
    /// `application/vnd.oci.empty.v1+json` or `application/vnd.pichi.config.v1+json`.
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// SHA-256 digest of the config blob.
    pub digest: Digest,
    /// Byte length of the config blob.
    pub size: u64,
    /// Base64-encoded inline blob (present for the empty config; omitted for a
    /// real config blob, which is stored/moved by digest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

impl ConfigDescriptor {
    /// The canonical OCI 1.1 empty-config descriptor (`{}` inline).
    pub fn canonical() -> Self {
        Self {
            media_type: MEDIA_TYPE_OCI_EMPTY_V1.to_string(),
            digest: EMPTY_CONFIG_DIGEST,
            size: 2,
            data: Some(EMPTY_CONFIG_DATA_BASE64.to_string()),
        }
    }

    /// A real config-blob descriptor (`vnd.pichi.config.v1+json`), referenced by
    /// digest; the blob is stored/moved separately.
    pub fn for_config(digest: Digest, size: u64) -> Self {
        Self {
            media_type: MEDIA_TYPE_PICHI_CONFIG_V1.to_string(),
            digest,
            size,
            data: None,
        }
    }

    /// `true` for the OCI empty config (no launch contract).
    pub fn is_empty(&self) -> bool {
        self.media_type == MEDIA_TYPE_OCI_EMPTY_V1
    }
}

/// Compile-time assertions: ensure `#[serde(rename = ...)]` strings on
/// `Layer` match the corresponding media-type constants. Any drift
/// (e.g. constant value updated without updating the serde attribute) will
/// cause a build error here.
const _: () = {
    assert!(
        const_str_eq(MEDIA_TYPE_PICHI_SCUTE_V1, "application/vnd.pichi.scute.v1"),
        "Layer::Scute serde rename must match MEDIA_TYPE_PICHI_SCUTE_V1"
    );
    assert!(
        const_str_eq(
            MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
            "application/vnd.pichi.scute.v1+zstd"
        ),
        "Layer::ScuteZstd serde rename must match MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD"
    );
    assert!(
        const_str_eq(MEDIA_TYPE_PICHI_PMI_V1, "application/vnd.pichi.pmi.v1"),
        "Layer::Pmi serde rename must match MEDIA_TYPE_PICHI_PMI_V1"
    );
    assert!(
        const_str_eq(
            MEDIA_TYPE_PICHI_PMI_V1_ZSTD,
            "application/vnd.pichi.pmi.v1+zstd"
        ),
        "Layer::PmiZstd serde rename must match its media type"
    );
    assert!(
        const_str_eq(
            MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD,
            "application/vnd.pichi.bootconfig.v1+zstd"
        ),
        "bootconfig serde rename must match its media type"
    );
};

/// Const-compatible byte-by-byte string equality for compile-time assertions.
const fn const_str_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Typed layer descriptor — internally-tagged on `mediaType`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "mediaType")]
pub enum Layer {
    /// Uncompressed scute (dm-snapshot COW) layer.
    #[serde(rename = "application/vnd.pichi.scute.v1")]
    Scute(ScuteDescriptor),
    /// Zstd-compressed scute layer (per D-08).
    #[serde(rename = "application/vnd.pichi.scute.v1+zstd")]
    ScuteZstd(ScuteDescriptor),

    /// Uncompressed PMI payload layer.
    #[serde(rename = "application/vnd.pichi.pmi.v1")]
    Pmi(PmiDescriptor),

    /// Zstd-compressed PMI transport; descriptor digest and size cover encoded bytes.
    #[serde(rename = "application/vnd.pichi.pmi.v1+zstd")]
    PmiZstd(PmiDescriptor),

    /// Zstd-compressed native boot configuration. Requires one PMI.
    #[serde(rename = "application/vnd.pichi.bootconfig.v1+zstd")]
    BootConfig(BootConfigDescriptor),
}

impl Layer {
    /// The content identity, regardless of layer encoding.
    pub fn digest(&self) -> &Digest {
        match self {
            Self::Scute(d) | Self::ScuteZstd(d) => &d.digest,
            Self::Pmi(d) | Self::PmiZstd(d) => &d.digest,
            Self::BootConfig(d) => &d.digest,
        }
    }

    /// Return the descriptor size regardless of variant.
    pub fn size(&self) -> u64 {
        match self {
            Self::Scute(d) | Self::ScuteZstd(d) => d.size,
            Self::Pmi(d) | Self::PmiZstd(d) => d.size,
            Self::BootConfig(d) => d.size,
        }
    }

    /// Return `true` if this layer's mediaType ends in `+zstd`. Used by
    /// Phase 44 cmd::pull's pipeline composer to decide whether to wrap
    /// the `BlobStore`-side sink in a `ZstdDecodeWriter` (Pitfall 5/12).
    pub fn is_zstd_variant(&self) -> bool {
        matches!(
            self,
            Self::ScuteZstd(_) | Self::PmiZstd(_) | Self::BootConfig(_)
        )
    }

    /// Return the static mediaType string for this variant. Mirrors the
    /// `#[serde(rename = "...")]` attribute literals so callers do not
    /// need to switch on the enum tag separately.
    pub fn media_type_str(&self) -> &'static str {
        match self {
            Self::Scute(_) => MEDIA_TYPE_PICHI_SCUTE_V1,
            Self::ScuteZstd(_) => MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
            Self::Pmi(_) => MEDIA_TYPE_PICHI_PMI_V1,
            Self::PmiZstd(_) => MEDIA_TYPE_PICHI_PMI_V1_ZSTD,
            Self::BootConfig(_) => MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD,
        }
    }
}

/// Scute layer descriptor (used by both `Scute` and `ScuteZstd` variants).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScuteDescriptor {
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
    /// Content digest of the PMI blob.
    pub digest: Digest,
    /// Byte size of the PMI blob.
    pub size: u64,
}

/// Validation errors for [`Manifest::validate`] (per D-07).
#[derive(Debug, Error)]
pub enum ManifestValidationError {
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
        self.layers
            .iter()
            .any(|l| matches!(l, Layer::Scute(_) | Layer::ScuteZstd(_)))
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
        let is_canonical_empty = c.media_type == MEDIA_TYPE_OCI_EMPTY_V1
            && c.size == 2
            && c.data.as_deref() == Some(EMPTY_CONFIG_DATA_BASE64)
            && c.digest == EMPTY_CONFIG_DIGEST;
        let is_config_blob = c.media_type == MEDIA_TYPE_PICHI_CONFIG_V1;
        if !is_canonical_empty && !is_config_blob {
            return Err(ManifestValidationError::BadConfig {
                media_type: c.media_type.clone(),
            });
        }
        self.carapace_root()?;
        // Rule 5 (out of order — bail early on count): at most one PMI
        let pmi_count = self
            .layers
            .iter()
            .filter(|l| matches!(l, Layer::Pmi(_) | Layer::PmiZstd(_)))
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
            let (Layer::Scute(d) | Layer::ScuteZstd(d)) = layer else {
                continue;
            };
            let blocks = d.annotations.data_blocks.get();
            let block_size = self.annotations.verity.data_block_size.bytes().get();
            if blocks.checked_mul(u64::from(block_size)).is_none()
                || (matches!(layer, Layer::Scute(_))
                    && blocks != d.size.div_ceil(u64::from(block_size)))
            {
                return Err(ManifestValidationError::BadScuteBlocks(d.digest.clone()));
            }
            if matches!(layer, Layer::Scute(_)) && d.annotations.digest != d.digest {
                return Err(ManifestValidationError::BadScuteAnnotation {
                    key: "dev.pichi.scute.digest",
                    value: d.annotations.digest.to_string(),
                });
            }
            let size = d.annotations.size.get();
            if size.div_ceil(u64::from(block_size)) != blocks
                || (matches!(layer, Layer::Scute(_)) && size != d.size)
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

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest as _, Sha256};
    use std::num::NonZeroU64;

    #[test]
    fn pmi_encodings_share_cardinality_and_bootconfig_rules() {
        let descriptor = PmiDescriptor {
            digest: Digest::Sha256([0x12; 32]),
            size: 73,
        };
        let variants = [Layer::Pmi(descriptor.clone()), Layer::PmiZstd(descriptor)];
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
                    manifest.annotations = Default::default();
                }
                manifest.layers.push(encoded.clone());
                manifest
                    .layers
                    .push(Layer::BootConfig(BootConfigDescriptor {
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
        assert_eq!(config.media_type_str(), MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD);

        for root in [false, true] {
            let mut manifest = sample_base_manifest();
            if !root {
                manifest.layers.clear();
                manifest.annotations = Default::default();
            }
            manifest.layers.push(config.clone());
            assert!(matches!(
                manifest.validate(),
                Err(ManifestValidationError::BootConfigWithoutPmi)
            ));
            manifest.layers.push(Layer::Pmi(PmiDescriptor {
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
        manifest.layers[0] = Layer::ScuteZstd(compressed);
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
        manifest.layers.push(Layer::ScuteZstd(scute));
        assert!(manifest.validate().is_ok());
        let Layer::ScuteZstd(scute) = &mut manifest.layers[0] else {
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
            media_type: "application/vnd.oci.image.manifest.v1+json".into(),
            artifact_type: MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
            config: ConfigDescriptor::canonical(),
            layers: vec![Layer::Scute(ScuteDescriptor {
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
    fn empty_config_canonical_matches_oci_spec() {
        let c = ConfigDescriptor::canonical();
        assert_eq!(c.media_type, MEDIA_TYPE_OCI_EMPTY_V1);
        assert_eq!(c.size, 2);
        assert_eq!(c.data.as_deref(), Some("e30="));
        assert_eq!(c.digest, EMPTY_CONFIG_DIGEST);
        // Cross-check: SHA-256 of "{}" really is the EMPTY_CONFIG_DIGEST hex
        let manual = Digest::Sha256(Sha256::digest(b"{}").into());
        assert_eq!(manual, EMPTY_CONFIG_DIGEST);
    }

    #[test]
    fn validate_accepts_base_manifest() {
        sample_base_manifest().validate().unwrap();
    }

    #[test]
    fn validate_accepts_bootable_image_with_one_pmi() {
        let mut m = sample_base_manifest();
        m.layers.push(Layer::Pmi(PmiDescriptor {
            digest: Digest::Sha256([0x22; 32]),
            size: 4096,
        }));
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
        m.config.media_type = "application/vnd.oci.image.config.v1+json".into();
        assert!(matches!(
            m.validate().unwrap_err(),
            ManifestValidationError::BadConfig { .. }
        ));
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
                digest: Digest::Sha256([0x44; 32]),
                size: 8192,
            }),
        );
        manifest.layers.push(Layer::ScuteZstd(child));
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
        m.annotations = Default::default();
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
            digest: Digest::Sha256([0x22; 32]),
            size: 4096,
        }));
        m.annotations = Default::default();
        m.validate().unwrap();
    }

    #[test]
    fn deserialize_unknown_layer_media_type_errors() {
        let json = r#"{"mediaType":"application/vnd.unknown.v1","digest":"sha256:1234","size":1}"#;
        let r: Result<Layer, _> = serde_json::from_str(json);
        assert!(r.is_err(), "unknown mediaType must fail to deserialize");
    }

    #[test]
    fn media_type_constant_used() {
        // Exercises MEDIA_TYPE_PICHI_ARTIFACT_V1 to satisfy dead_code lint.
        assert!(MEDIA_TYPE_PICHI_ARTIFACT_V1.starts_with("application/"));
    }

    #[test]
    fn pichi_layer_is_zstd_variant() {
        let scute_desc = ScuteDescriptor {
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
            digest: Digest::Sha256([0xbe; 32]),
            size: 0,
        };
        assert!(!Layer::Scute(scute_desc.clone()).is_zstd_variant());
        assert!(Layer::ScuteZstd(scute_desc).is_zstd_variant());
        assert!(!Layer::Pmi(pmi_desc).is_zstd_variant());
    }

    #[test]
    fn pichi_layer_media_type_str() {
        let scute_desc = ScuteDescriptor {
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
            digest: Digest::Sha256([0xbe; 32]),
            size: 0,
        };
        assert_eq!(
            Layer::Scute(scute_desc.clone()).media_type_str(),
            MEDIA_TYPE_PICHI_SCUTE_V1
        );
        assert_eq!(
            Layer::ScuteZstd(scute_desc).media_type_str(),
            MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD
        );
        assert_eq!(
            Layer::Pmi(pmi_desc).media_type_str(),
            MEDIA_TYPE_PICHI_PMI_V1
        );
    }
}
