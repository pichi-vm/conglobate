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

use crate::artifact::annotation::{
    ANNOTATION_CARAPACE_VERITY_ALGO, ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE,
    ANNOTATION_CARAPACE_VERITY_HASH_BLOCK_SIZE, ANNOTATION_CARAPACE_VERITY_HASH_TYPE,
    ANNOTATION_CARAPACE_VERITY_VERSION, ANNOTATION_SCUTE_VERITY_DATA_BLOCKS,
    ANNOTATION_SCUTE_VERITY_SALT,
};
use crate::artifact::media_type::{
    MEDIA_TYPE_OCI_EMPTY_V1, MEDIA_TYPE_PICHI_ARTIFACT_V1, MEDIA_TYPE_PICHI_CONFIG_V1,
    MEDIA_TYPE_PICHI_DTB_V1, MEDIA_TYPE_PICHI_PMI_V1, MEDIA_TYPE_PICHI_SCUTE_V1,
    MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
};

// The carapace verity annotations below are the dm-verity superblock fields
// a reader needs to rebuild the verity layer, which is not distributed. All
// of them must be present when the manifest has scute layers.
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
/// - Layer order is NOT load-bearing (D-03); writers MAY emit in any order.
/// - At most ONE [`Layer::Pmi`] (D-03 — zero pmi = base/non-bootable, one pmi = appliance/bootable).
/// - Top-level annotations MUST include the carapace verity *param* keys
///   (D-06 / D-07 rule 3) when scute layers exist. Tip hash is not used.
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
    /// Typed layer descriptors. Order is NOT load-bearing (D-03).
    pub layers: Vec<Layer>,
    /// Top-level annotations. MUST include `dev.pichi.carapace.verity.{algo,
    /// data-block-size,hash-block-size}` and SHOULD include
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
    assert!(
        const_str_eq(
            ANNOTATION_SCUTE_VERITY_DATA_BLOCKS,
            "dev.pichi.scute.verity.data-blocks"
        ),
        "ScuteAnnotations::data_blocks serde rename must match its constant"
    );
    assert!(
        const_str_eq(ANNOTATION_SCUTE_VERITY_SALT, "dev.pichi.scute.verity.salt"),
        "ScuteAnnotations::salt serde rename must match its constant"
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
    /// Optional for manifests published before streaming formatting.
    #[serde(
        rename = "dev.pichi.scute.verity.data-blocks",
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "ScuteAnnotations::serialize_blocks",
        deserialize_with = "ScuteAnnotations::deserialize_blocks"
    )]
    pub data_blocks: Option<u64>,
    /// Hex-encoded salt for this scute.
    #[serde(rename = "dev.pichi.scute.verity.salt")]
    pub salt: String,
}

impl ScuteAnnotations {
    // OCI annotation values are strings; expose a typed length to callers.
    #[allow(clippy::ref_option)] // Signature required by serde serialize_with.
    fn serialize_blocks<S: serde::Serializer>(
        blocks: &Option<u64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        blocks.map(|n| n.to_string()).serialize(serializer)
    }

    fn deserialize_blocks<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u64>, D::Error> {
        let value = String::deserialize(deserializer)?;
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(serde::de::Error::custom(
                "data_blocks must be an unsigned decimal string",
            ));
        }
        value.parse().map(Some).map_err(serde::de::Error::custom)
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
    /// D-07 rule 4: scute salt is not valid hex.
    #[error("scute salt is not valid hex: {0}")]
    BadSalt(String),

    /// Declared data-block count is zero or contradicts a raw blob's size.
    #[error("invalid data-block count for scute {0}")]
    BadScuteBlocks(String),

    /// More than one base DTB layer (BUILD.md §7.1 — at most one).
    #[error("artifact has more than one base DTB layer (got {0}); at most one is permitted")]
    MultipleDtb(usize),
    /// A helper rejected zstd-compressed scute layers (decompressed-COW deferred).
    #[error("artifact has zstd-compressed scute layers; decompressed-COW handling is deferred")]
    CompressedScuteUnsupported,
    /// A bootable partition helper found no PMI layer.
    #[error("artifact is not bootable (no PMI layer); usable as a from: source only")]
    MissingPmi,
}

impl Manifest {
    /// Deserialise a manifest from a JSON byte stream. Does NOT validate.
    /// Use [`Self::from_reader_validated`] to deserialise + validate in one call.
    ///
    /// # Errors
    /// Returns [`crate::artifact::Error::Json`] or [`crate::artifact::Error::Io`] on failure.
    pub fn from_reader<R: std::io::Read>(r: R) -> Result<Self, crate::artifact::Error> {
        Ok(serde_json::from_reader(r)?)
    }

    /// Deserialise + validate (per D-07).
    ///
    /// # Errors
    /// Returns [`crate::artifact::Error`] on parse failure or `Err(crate::artifact::Error::Validation(...))`
    /// on D-07 rule violation.
    pub fn from_reader_validated<R: std::io::Read>(r: R) -> Result<Self, crate::artifact::Error> {
        let m = Self::from_reader(r)?;
        m.validate()?;
        Ok(m)
    }

    /// Serialise this manifest to a compact JSON byte vector. Byte-stable
    /// modulo annotation key order (BTreeMap) and struct field declaration
    /// order (serde derive guarantee).
    ///
    /// # Errors
    /// Returns [`crate::artifact::Error::Json`] on serialisation failure.
    pub fn to_bytes(&self) -> Result<Vec<u8>, crate::artifact::Error> {
        Ok(serde_json::to_vec(self)?)
    }

    /// Compute the SHA-256 content digest of the serialised manifest.
    ///
    /// # Errors
    /// Returns [`crate::artifact::Error`] if serialisation fails.
    pub fn digest(&self) -> Result<crate::artifact::Digest, crate::artifact::Error> {
        let bytes = self.to_bytes()?;
        Ok(crate::artifact::Digest::from_bytes_sha256(&bytes))
    }

    /// Return `true` if this manifest carries at least one scute layer (i.e.
    /// it describes a carapace, so a verity root anchor is meaningful).
    fn has_scutes(&self) -> bool {
        self.layers
            .iter()
            .any(|l| matches!(l, Layer::Scute(_) | Layer::ScuteZstd(_)))
    }

    /// The single base DTB layer, if present.
    pub fn dtb_layer(&self) -> Option<&DtbDescriptor> {
        self.layers.iter().find_map(|layer| match layer {
            Layer::Dtb(descriptor) => Some(descriptor),
            _ => None,
        })
    }

    /// Partition layers into the PMI descriptor and ordered scute list.
    ///
    /// Fails when the artifact has no PMI (not bootable) or carries zstd
    /// scutes (decompressed-COW handling is deferred). DTB layers are ignored.
    pub fn partition_layers(
        &self,
    ) -> Result<(&PmiDescriptor, Vec<&ScuteDescriptor>), ManifestValidationError> {
        let mut pmi: Option<&PmiDescriptor> = None;
        let mut scutes = Vec::new();
        for layer in &self.layers {
            match layer {
                Layer::Pmi(descriptor) => pmi = Some(descriptor),
                Layer::Scute(descriptor) => scutes.push(descriptor),
                Layer::Dtb(_) => {}
                Layer::ScuteZstd(_) => {
                    return Err(ManifestValidationError::CompressedScuteUnsupported);
                }
            }
        }
        let pmi = pmi.ok_or(ManifestValidationError::MissingPmi)?;
        Ok((pmi, scutes))
    }

    /// Collect uncompressed scute layers (build `from:` sources).
    ///
    /// Fails when any zstd scute is present (decompressed-COW deferred).
    pub fn scute_layers(&self) -> Result<Vec<&ScuteDescriptor>, ManifestValidationError> {
        let mut scutes = Vec::new();
        for layer in &self.layers {
            match layer {
                Layer::Scute(descriptor) => scutes.push(descriptor),
                Layer::Pmi(_) | Layer::Dtb(_) => {}
                Layer::ScuteZstd(_) => {
                    return Err(ManifestValidationError::CompressedScuteUnsupported);
                }
            }
        }
        Ok(scutes)
    }

    /// Validate per D-07. See [`ManifestValidationError`] for the rule set.
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
        if self.has_scutes() {
            for key in [
                ANNOTATION_CARAPACE_VERITY_ALGO,
                ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE,
                ANNOTATION_CARAPACE_VERITY_HASH_BLOCK_SIZE,
                ANNOTATION_CARAPACE_VERITY_VERSION,
                ANNOTATION_CARAPACE_VERITY_HASH_TYPE,
            ] {
                if !self.annotations.contains_key(key) {
                    return Err(ManifestValidationError::MissingChainAnnotation(key));
                }
            }
            // Well-formedness only (no value whitelist — that is the guest
            // consumer's TCB control per the carapace spec, not storage's):
            // version / hash-type must parse as integers.
            for key in [
                ANNOTATION_CARAPACE_VERITY_VERSION,
                ANNOTATION_CARAPACE_VERITY_HASH_TYPE,
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
        // Rule 4: every scute has a hex-parseable salt (presence enforced by
        // the typed `ScuteAnnotations`; here we check hex validity)
        for layer in &self.layers {
            let (Layer::Scute(d) | Layer::ScuteZstd(d)) = layer else {
                continue;
            };
            if let Some(blocks) = d.annotations.data_blocks {
                let value = &self.annotations[ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE];
                let block_size = value
                    .parse::<u32>()
                    .ok()
                    .filter(|size| *size != 0)
                    .ok_or_else(|| ManifestValidationError::BadChainAnnotation {
                        key: ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE,
                        value: value.clone(),
                    })?;
                if blocks == 0
                    || (matches!(layer, Layer::Scute(_))
                        && blocks != d.size.div_ceil(u64::from(block_size)))
                {
                    return Err(ManifestValidationError::BadScuteBlocks(d.digest.clone()));
                }
            }
            if crate::hex::decode(&d.annotations.salt).is_none() {
                return Err(ManifestValidationError::BadSalt(d.annotations.salt.clone()));
            }
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

    #[test]
    fn data_blocks_is_an_optional_decimal_annotation() {
        let legacy: ScuteAnnotations =
            serde_json::from_str(r#"{"dev.pichi.scute.verity.salt":"00"}"#).unwrap();
        assert_eq!(legacy.data_blocks, None);
        assert!(
            !serde_json::to_value(&legacy)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("dev.pichi.scute.verity.data-blocks")
        );
        let annotations = ScuteAnnotations {
            salt: "00".into(),
            data_blocks: Some(2),
        };
        let json = serde_json::to_value(&annotations).unwrap();
        assert_eq!(json["dev.pichi.scute.verity.data-blocks"], "2");
        assert_eq!(
            serde_json::from_value::<ScuteAnnotations>(json.clone()).unwrap(),
            annotations
        );
        for invalid in [
            serde_json::json!(4097),
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
        for blocks in [0, 1, 2, 1024] {
            let mut manifest = sample_base_manifest();
            let Layer::Scute(scute) = &mut manifest.layers[0] else {
                unreachable!()
            };
            scute.annotations.data_blocks = Some(blocks);
            assert_eq!(manifest.validate().is_ok(), blocks == 1);
        }
        let mut manifest = sample_base_manifest();
        let Layer::Scute(mut scute) = manifest.layers.remove(0) else {
            unreachable!()
        };
        scute.annotations.data_blocks = Some(2);
        manifest.layers.push(Layer::ScuteZstd(scute));
        assert!(manifest.validate().is_ok());
    }

    fn sample_chain_annotations() -> BTreeMap<String, String> {
        let mut a = BTreeMap::new();
        a.insert(ANNOTATION_CARAPACE_VERITY_ALGO.into(), "sha256".into());
        a.insert(
            ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE.into(),
            "4096".into(),
        );
        a.insert(
            ANNOTATION_CARAPACE_VERITY_HASH_BLOCK_SIZE.into(),
            "4096".into(),
        );
        a.insert(ANNOTATION_CARAPACE_VERITY_VERSION.into(), "1".into());
        a.insert(ANNOTATION_CARAPACE_VERITY_HASH_TYPE.into(), "1".into());
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
                    data_blocks: None,
                    salt: "00ff".into(),
                },
            })],
            annotations: sample_chain_annotations(),
        }
    }

    #[test]
    fn round_trip() {
        let m = sample_base_manifest();
        let b = m.to_bytes().unwrap();
        let m2 = Manifest::from_reader(b.as_slice()).unwrap();
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

        let b = m.to_bytes().unwrap();
        let m2 = Manifest::from_reader_validated(b.as_slice()).unwrap();
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
        let b1 = m.to_bytes().unwrap();
        let m2 = Manifest::from_reader(b1.as_slice()).unwrap();
        let b2 = m2.to_bytes().unwrap();
        assert_eq!(b1, b2, "manifest bytes must be stable across round-trip");
    }

    #[test]
    fn digest_matches_to_bytes_hash() {
        let m = sample_base_manifest();
        let bytes = m.to_bytes().unwrap();
        assert_eq!(
            m.digest().unwrap(),
            crate::artifact::Digest::from_bytes_sha256(&bytes)
        );
    }

    #[test]
    fn empty_config_canonical_matches_oci_spec() {
        let c = ConfigDescriptor::canonical();
        assert_eq!(c.media_type, MEDIA_TYPE_OCI_EMPTY_V1);
        assert_eq!(c.size, 2);
        assert_eq!(c.data.as_deref(), Some("e30="));
        assert_eq!(c.digest, EMPTY_CONFIG_DIGEST);
        // Cross-check: SHA-256 of "{}" really is the EMPTY_CONFIG_DIGEST hex
        let manual = crate::artifact::Digest::from_bytes_sha256(b"{}").to_string();
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
        m.annotations.remove(ANNOTATION_CARAPACE_VERITY_ALGO);
        let err = m.validate().unwrap_err();
        assert!(
            matches!(err, ManifestValidationError::MissingChainAnnotation(k) if k == ANNOTATION_CARAPACE_VERITY_ALGO)
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
    fn validate_ignores_stale_tip_hash_annotation() {
        // Tip hash is not read: a present stale label must not affect validate.
        let mut m = sample_base_manifest();
        m.annotations
            .insert("dev.pichi.carapace.verity.hash".into(), "deadbeef".into());
        m.validate().unwrap();
    }

    #[test]
    fn validate_requires_verity_version_and_hash_type_when_scutes_present() {
        for key in [
            ANNOTATION_CARAPACE_VERITY_VERSION,
            ANNOTATION_CARAPACE_VERITY_HASH_TYPE,
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
            ANNOTATION_CARAPACE_VERITY_VERSION.into(),
            "not-a-number".into(),
        );
        assert!(matches!(
            m.validate().unwrap_err(),
            ManifestValidationError::BadChainAnnotation { key, .. }
                if key == ANNOTATION_CARAPACE_VERITY_VERSION
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
    fn dtb_layer_returns_single_base_dtb() {
        let mut m = sample_base_manifest();
        assert!(m.dtb_layer().is_none());
        m.layers.push(sample_dtb_layer());
        assert_eq!(m.dtb_layer().unwrap().size, 8192);
    }

    #[test]
    fn partition_layers_splits_pmi_and_scutes() {
        let mut m = sample_base_manifest();
        m.layers.push(Layer::Pmi(PmiDescriptor {
            digest: "sha256:2222222222222222222222222222222222222222222222222222222222222222"
                .into(),
            size: 4096,
        }));
        m.layers.push(sample_dtb_layer());
        let (pmi, scutes) = m.partition_layers().unwrap();
        assert_eq!(pmi.size, 4096);
        assert_eq!(scutes.len(), 1);
    }

    #[test]
    fn partition_layers_requires_pmi() {
        let m = sample_base_manifest();
        assert!(matches!(
            m.partition_layers().unwrap_err(),
            ManifestValidationError::MissingPmi
        ));
    }

    #[test]
    fn scute_layers_collects_uncompressed_scutes() {
        let m = sample_base_manifest();
        assert_eq!(m.scute_layers().unwrap().len(), 1);
    }

    #[test]
    fn scute_layers_rejects_zstd() {
        let mut m = sample_base_manifest();
        if let Layer::Scute(descriptor) = m.layers[0].clone() {
            m.layers[0] = Layer::ScuteZstd(descriptor);
        }
        assert!(matches!(
            m.scute_layers().unwrap_err(),
            ManifestValidationError::CompressedScuteUnsupported
        ));
    }

    #[test]
    fn deserialize_unknown_layer_media_type_errors() {
        let json = r#"{"mediaType":"application/vnd.unknown.v1","digest":"sha256:1234","size":1}"#;
        let r: Result<Layer, _> = serde_json::from_str(json);
        assert!(r.is_err(), "unknown mediaType must fail to deserialize");
    }

    #[test]
    fn from_reader_validated_combines_parse_and_validate() {
        let m = sample_base_manifest();
        let b = m.to_bytes().unwrap();
        let m2 = Manifest::from_reader_validated(b.as_slice()).unwrap();
        assert_eq!(m, m2);
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
                data_blocks: None,
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
                data_blocks: None,
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
