// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! `Manifest` typed schema (Phase 42 FORMAT-01..03). Per D-03, manifests
//! are content-addressed JSON stored in the blob store; this type provides
//! parse / serialise / validate / self-digest helpers.
//!
//! Phase 42 locks the schema: typed `layers: Vec<Layer>` (internally-
//! tagged on `mediaType`), `ConfigDescriptor` for the OCI 1.1 empty-config
//! pattern, and `validate()` enforcing all six D-07 rules.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::media_type::{
    MEDIA_TYPE_OCI_EMPTY_V1, MEDIA_TYPE_PICHI_ARTIFACT_V1, MEDIA_TYPE_PICHI_CONFIG_V1,
    MEDIA_TYPE_PICHI_DTB_V1, MEDIA_TYPE_PICHI_PMI_V1, MEDIA_TYPE_PICHI_SCUTE_V1,
    MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
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
/// - At most ONE [`Layer::Pmi`] (D-03 — zero pmi = base/non-bootable, one pmi = appliance/bootable).
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
    pub config: ConfigDescriptor,
    /// Typed layer descriptors. Scutes appear in base-to-tip chain order;
    /// PMI and DTB descriptors do not participate in that order.
    pub layers: Vec<Layer>,
    /// Top-level annotations. With scutes, MUST include
    /// `dev.pichi.carapace.verity.{algo,data-block-size,hash-block-size,
    /// version,hash-type,hash}` and SHOULD include
    /// `org.opencontainers.image.created` (D-06).
    /// `BTreeMap` for byte-stable JSON serialisation (lexicographic key order).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
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
    pub digest: String,
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
            digest: EMPTY_CONFIG_DIGEST.to_string(),
            size: 2,
            data: Some(EMPTY_CONFIG_DATA_BASE64.to_string()),
        }
    }

    /// A real config-blob descriptor (`vnd.pichi.config.v1+json`), referenced by
    /// digest; the blob is stored/moved separately.
    pub fn for_config(digest: String, size: u64) -> Self {
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
        const_str_eq(MEDIA_TYPE_PICHI_DTB_V1, "application/vnd.pichi.dtb.v1"),
        "Layer::Dtb serde rename must match MEDIA_TYPE_PICHI_DTB_V1"
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
    /// PMI payload layer (uncompressed per D-09).
    #[serde(rename = "application/vnd.pichi.pmi.v1")]
    Pmi(PmiDescriptor),
    /// Base DTB layer for a detached-channel PMI (BUILD.md §7.1). At most one.
    #[serde(rename = "application/vnd.pichi.dtb.v1")]
    Dtb(DtbDescriptor),
}

impl Layer {
    /// Return the descriptor digest as a `&str` regardless of variant.
    pub fn digest_str(&self) -> &str {
        match self {
            Self::Scute(d) | Self::ScuteZstd(d) => &d.digest,
            Self::Pmi(d) => &d.digest,
            Self::Dtb(d) => &d.digest,
        }
    }

    /// Return the descriptor size regardless of variant.
    pub fn size(&self) -> u64 {
        match self {
            Self::Scute(d) | Self::ScuteZstd(d) => d.size,
            Self::Pmi(d) => d.size,
            Self::Dtb(d) => d.size,
        }
    }

    /// Return `true` if this layer's mediaType ends in `+zstd`. Used by
    /// Phase 44 cmd::pull's pipeline composer to decide whether to wrap
    /// the `BlobStore`-side sink in a `ZstdDecodeWriter` (Pitfall 5/12).
    pub fn is_zstd_variant(&self) -> bool {
        matches!(self, Self::ScuteZstd(_))
    }

    /// Return the static mediaType string for this variant. Mirrors the
    /// `#[serde(rename = "...")]` attribute literals so callers do not
    /// need to switch on the enum tag separately.
    pub fn media_type_str(&self) -> &'static str {
        match self {
            Self::Scute(_) => MEDIA_TYPE_PICHI_SCUTE_V1,
            Self::ScuteZstd(_) => MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
            Self::Pmi(_) => MEDIA_TYPE_PICHI_PMI_V1,
            Self::Dtb(_) => MEDIA_TYPE_PICHI_DTB_V1,
        }
    }
}

/// Scute layer descriptor (used by both `Scute` and `ScuteZstd` variants).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScuteDescriptor {
    /// Content digest of the blob.
    pub digest: String,
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
    #[serde(
        rename = "dev.pichi.scute.verity.data-blocks",
        serialize_with = "ScuteAnnotations::serialize_blocks",
        deserialize_with = "ScuteAnnotations::deserialize_blocks"
    )]
    pub data_blocks: std::num::NonZeroU64,
    /// Hex-encoded salt for this scute.
    #[serde(rename = "dev.pichi.scute.verity.salt")]
    pub salt: String,
}

impl ScuteAnnotations {
    // OCI annotation values are strings; expose a typed length to callers.
    fn serialize_blocks<S: serde::Serializer>(
        blocks: &std::num::NonZeroU64,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(blocks)
    }

    fn deserialize_blocks<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<std::num::NonZeroU64, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(serde::de::Error::custom(
                "data_blocks must be an unsigned decimal string",
            ));
        }
        value.parse().map_err(serde::de::Error::custom)
    }
}

/// PMI layer descriptor (no annotations — D-09).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PmiDescriptor {
    /// Content digest of the PMI blob.
    pub digest: String,
    /// Byte size of the PMI blob.
    pub size: u64,
}

/// Base DTB layer descriptor (no annotations; the measured base devicetree for
/// a detached-channel PMI — BUILD.md §7.1).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DtbDescriptor {
    /// Content digest of the DTB blob.
    pub digest: String,
    /// Byte size of the DTB blob.
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
    /// Scute salt is not valid hex or lacks the 32-byte parent root prefix.
    #[error("scute salt must contain a hex-encoded parent root: {0}")]
    BadSalt(String),

    /// The first scute must be the base; later scutes must have a parent.
    #[error("scutes must appear in base-to-tip order: {0}")]
    BadScuteOrder(String),

    /// Declared data-block count is zero or contradicts a raw blob's size.
    #[error("invalid data-block count for scute {0}")]
    BadScuteBlocks(String),

    /// More than one base DTB layer (BUILD.md §7.1 — at most one).
    #[error("artifact has more than one base DTB layer (got {0}); at most one is permitted")]
    MultipleDtb(usize),
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
            .filter(|l| matches!(l, Layer::Pmi(_)))
            .count();
        if pmi_count > 1 {
            return Err(ManifestValidationError::MultiplePmi(pmi_count));
        }
        // At most one base DTB layer (BUILD.md §7.1).
        let dtb_count = self
            .layers
            .iter()
            .filter(|l| matches!(l, Layer::Dtb(_)))
            .count();
        if dtb_count > 1 {
            return Err(ManifestValidationError::MultipleDtb(dtb_count));
        }
        // The salt's parent prefix identifies the base without reading blobs.
        // Full chain continuity is checked when consumers verify the scutes.
        let mut first_scute = true;
        for layer in &self.layers {
            let (Layer::Scute(d) | Layer::ScuteZstd(d)) = layer else {
                continue;
            };
            let blocks = d.annotations.data_blocks.get();
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
                || (matches!(layer, Layer::Scute(_))
                    && blocks != d.size.div_ceil(u64::from(block_size)))
            {
                return Err(ManifestValidationError::BadScuteBlocks(d.digest.clone()));
            }
            let salt = hex::decode(&d.annotations.salt)
                .map_err(|_| ManifestValidationError::BadSalt(d.annotations.salt.clone()))?;
            let parent = salt
                .get(..32)
                .ok_or_else(|| ManifestValidationError::BadSalt(d.annotations.salt.clone()))?;
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

    #[test]
    fn data_blocks_is_a_required_positive_decimal_annotation() {
        assert!(
            serde_json::from_str::<ScuteAnnotations>(r#"{"dev.pichi.scute.verity.salt":"00"}"#)
                .is_err()
        );
        let annotations = ScuteAnnotations {
            salt: "00".into(),
            data_blocks: 2u64.try_into().unwrap(),
        };
        let json = serde_json::to_value(&annotations).unwrap();
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
            serde_json::json!("+1"),
            serde_json::json!("18446744073709551616"),
            serde_json::Value::Null,
        ] {
            let mut json = json.clone();
            json["dev.pichi.scute.verity.data-blocks"] = invalid;
            assert!(serde_json::from_value::<ScuteAnnotations>(json).is_err());
        }
    }

    #[test]
    fn validate_data_blocks_against_raw_descriptor() {
        for blocks in [1u64, 2, 1024, u64::MAX] {
            let mut manifest = sample_base_manifest();
            let Layer::Scute(scute) = &mut manifest.layers[0] else {
                unreachable!()
            };
            scute.annotations.data_blocks = blocks.try_into().unwrap();
            assert_eq!(manifest.validate().is_ok(), blocks == 1);
        }
        let mut manifest = sample_base_manifest();
        let Layer::Scute(mut scute) = manifest.layers.remove(0) else {
            unreachable!()
        };
        scute.annotations.data_blocks = 2u64.try_into().unwrap();
        manifest.layers.push(Layer::ScuteZstd(scute));
        assert!(manifest.validate().is_ok());
        let Layer::ScuteZstd(scute) = &mut manifest.layers[0] else {
            unreachable!()
        };
        scute.annotations.data_blocks = u64::MAX.try_into().unwrap();
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
            config: ConfigDescriptor::canonical(),
            layers: vec![Layer::Scute(ScuteDescriptor {
                digest: "sha256:1111111111111111111111111111111111111111111111111111111111111111"
                    .into(),
                size: 1024,
                annotations: ScuteAnnotations {
                    data_blocks: 1u64.try_into().unwrap(),
                    salt: "00".repeat(32),
                },
            })],
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

    fn sample_dtb_layer() -> Layer {
        Layer::Dtb(DtbDescriptor {
            digest: "sha256:4444444444444444444444444444444444444444444444444444444444444444"
                .into(),
            size: 8192,
        })
    }

    #[test]
    fn dtb_layer_round_trips_and_validates() {
        let mut m = sample_base_manifest();
        m.layers.push(Layer::Pmi(PmiDescriptor {
            digest: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .into(),
            size: 4096,
        }));
        m.layers.push(sample_dtb_layer());
        m.validate().unwrap();

        let b = serde_json::to_vec(&m).unwrap();
        let m2 = serde_json::from_slice::<Manifest>(&b).unwrap();
        m2.validate().unwrap();
        assert_eq!(m, m2);
        let json = String::from_utf8(b).unwrap();
        assert!(json.contains(MEDIA_TYPE_PICHI_DTB_V1));
    }

    #[test]
    fn validate_rejects_two_dtb_layers() {
        let mut m = sample_base_manifest();
        m.layers.push(sample_dtb_layer());
        m.layers.push(sample_dtb_layer());
        let err = m.validate().unwrap_err();
        assert!(
            matches!(err, ManifestValidationError::MultipleDtb(2)),
            "{err:?}"
        );
    }

    #[test]
    fn dtb_layer_media_type_and_digest() {
        let l = sample_dtb_layer();
        assert_eq!(l.media_type_str(), MEDIA_TYPE_PICHI_DTB_V1);
        assert_eq!(l.size(), 8192);
        assert!(l.digest_str().starts_with("sha256:44"));
        assert!(!l.is_zstd_variant());
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
        let manual = crate::artifact::Digest::Sha256(Sha256::digest(b"{}").into()).to_string();
        assert_eq!(manual, EMPTY_CONFIG_DIGEST);
    }

    #[test]
    fn validate_accepts_base_manifest() {
        sample_base_manifest().validate().unwrap();
    }

    #[test]
    fn validate_accepts_appliance_with_one_pmi() {
        let mut m = sample_base_manifest();
        m.layers.push(Layer::Pmi(PmiDescriptor {
            digest: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .into(),
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
        m.config = ConfigDescriptor::for_config(
            "sha256:5555555555555555555555555555555555555555555555555555555555555555".into(),
            42,
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
            m.layers.push(Layer::Pmi(PmiDescriptor {
                digest: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                    .into(),
                size: 1,
            }));
        }
        assert!(matches!(
            m.validate().unwrap_err(),
            ManifestValidationError::MultiplePmi(2)
        ));
    }

    #[test]
    fn validate_rejects_non_hex_salt() {
        let mut m = sample_base_manifest();
        if let Layer::Scute(d) = &mut m.layers[0] {
            d.annotations.salt = "ZZZZ".into();
        }
        assert!(matches!(
            m.validate().unwrap_err(),
            ManifestValidationError::BadSalt(_)
        ));
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
        base.annotations.salt.push_str("abcd");
        child.annotations.salt = "ab".repeat(32);
        manifest.layers.insert(0, sample_dtb_layer());
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
            base.annotations.salt = salt;
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
        m.layers.push(Layer::Pmi(PmiDescriptor {
            digest: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .into(),
            size: 4096,
        }));
        m.annotations.clear();
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
            digest: "sha256:dead".into(),
            size: 0,
            annotations: ScuteAnnotations {
                data_blocks: 1u64.try_into().unwrap(),
                salt: "deadbeef".into(),
            },
        };
        let pmi_desc = PmiDescriptor {
            digest: "sha256:beef".into(),
            size: 0,
        };
        assert!(!Layer::Scute(scute_desc.clone()).is_zstd_variant());
        assert!(Layer::ScuteZstd(scute_desc).is_zstd_variant());
        assert!(!Layer::Pmi(pmi_desc).is_zstd_variant());
    }

    #[test]
    fn pichi_layer_media_type_str() {
        let scute_desc = ScuteDescriptor {
            digest: "sha256:dead".into(),
            size: 0,
            annotations: ScuteAnnotations {
                data_blocks: 1u64.try_into().unwrap(),
                salt: "deadbeef".into(),
            },
        };
        let pmi_desc = PmiDescriptor {
            digest: "sha256:beef".into(),
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
