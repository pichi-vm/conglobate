// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Media-type string constants. Defined in Phase 41; Phase 42 (FORMAT-01..03)
//! locks the final set per CONTEXT.md D-03/D-04/D-08/D-09:
//!
//! - Drop `scute-cow.v1` (per D-03 — there's no scute-verity to disambiguate
//!   against, so the cow IS the entire scute on the wire).
//! - Drop `scute-verity.v1` (per D-04 — verity blobs are not distributed;
//!   they're recomputed deterministically during pull from cow + salt + chain
//!   params).
//! - Add `scute.v1+zstd` (per D-08 — cow blobs MAY be zstd-compressed; default
//!   level 3; readers must handle either form).
//! - PMI transport accepts raw bytes or a zstd frame; launchers consume the decoded PMI.

use super::Media;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::marker::PhantomData;

/// A required wire string whose value is fixed by its format type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaType<T: Media>(PhantomData<T>);

impl<T: Media> Default for MediaType<T> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<T: Media> Serialize for MediaType<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(T::MEDIA_TYPE)
    }
}

impl<'de, T: Media> Deserialize<'de> for MediaType<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if String::deserialize(deserializer)? != T::MEDIA_TYPE {
            return Err(de::Error::custom(format_args!(
                "expected media type {}",
                T::MEDIA_TYPE
            )));
        }
        Ok(Self(PhantomData))
    }
}

/// Zstd-compressed PMI transport. The descriptor identifies the encoded bytes;
/// the consumer verifies and decodes them before passing the PMI to a launcher.
pub const MEDIA_TYPE_PICHI_PMI_V1_ZSTD: &str = "application/vnd.pichi.pmi.v1+zstd";

/// `requirements.yaml` launch-contract layer (BUILD.md §7) — the host-facing
/// resource contract for a launchable artifact. A small uncompressed YAML
/// blob; at most one per manifest.
///
/// Superseded by the config blob ([`MEDIA_TYPE_PICHI_CONFIG_V1`]): the launch
/// contract now rides in the manifest config, not a layer. Retained during the
/// migration; new writers MUST NOT emit it.
pub const MEDIA_TYPE_PICHI_REQUIREMENTS_V1: &str = "application/vnd.pichi.requirements.v1+yaml";

/// Zstd-compressed native Linux boot configuration text accompanying a PMI.
pub const MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD: &str = "application/vnd.pichi.bootconfig.v1+zstd";

/// pichi image config blob — the OCI 1.1 `config` descriptor's target. Carries
/// the launch contract (the `requirements` projection of `config.yaml`,
/// BUILD.md §7.1); replaces the OCI empty-config for pichi artifacts. JSON.
pub const MEDIA_TYPE_PICHI_CONFIG_V1: &str = "application/vnd.pichi.config.v1+json";

/// OCI 1.1 empty descriptor. Formerly the artifact's config blob (inline
/// `data: "e30="`); pichi artifacts now use [`MEDIA_TYPE_PICHI_CONFIG_V1`].
pub const MEDIA_TYPE_OCI_EMPTY_V1: &str = "application/vnd.oci.empty.v1+json";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::PichiArtifact;
    use crate::artifact::PmiDescriptor;
    use crate::artifact::ScuteDescriptor;

    struct Artifact;
    impl Media for Artifact {
        const MEDIA_TYPE: &'static str = "application/vnd.example.artifact";
    }

    struct Pmi;
    impl Media for Pmi {
        const MEDIA_TYPE: &'static str = "application/vnd.pichi.pmi.v1";
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Descriptor {
        Artifact {
            #[serde(rename = "mediaType")]
            media_type: MediaType<Artifact>,
        },
        Pmi {
            #[serde(rename = "mediaType")]
            media_type: MediaType<Pmi>,
        },
    }

    #[test]
    fn fixed_media_type_discriminates_untagged_variants() {
        for media in [Artifact::MEDIA_TYPE, Pmi::MEDIA_TYPE] {
            let json = serde_json::json!({"mediaType": media});
            let value: Descriptor = serde_json::from_value(json).unwrap();
            let encoded = match value {
                Descriptor::Artifact { media_type } => serde_json::to_value(media_type).unwrap(),
                Descriptor::Pmi { media_type } => serde_json::to_value(media_type).unwrap(),
            };
            assert_eq!(encoded, media);
        }
        for invalid in [
            serde_json::json!({}),
            serde_json::json!({"mediaType": "other"}),
            serde_json::json!({"mediaType": null}),
            serde_json::json!({"mediaType": 1}),
        ] {
            assert!(serde_json::from_value::<Descriptor>(invalid).is_err());
        }
    }

    #[test]
    fn frozen_wire_strings_match_phase_42_lock() {
        // Exact wire strings — any drift breaks OCI consumers.
        assert_eq!(
            PichiArtifact::MEDIA_TYPE,
            "application/vnd.pichi.artifact.v1+json"
        );
        assert_eq!(
            ScuteDescriptor::MEDIA_TYPE,
            "application/vnd.pichi.scute.v1"
        );
        assert_eq!(
            crate::artifact::Zstd::<crate::artifact::ScuteDescriptor>::MEDIA_TYPE,
            "application/vnd.pichi.scute.v1+zstd"
        );
        assert_eq!(PmiDescriptor::MEDIA_TYPE, "application/vnd.pichi.pmi.v1");
        assert_eq!(
            MEDIA_TYPE_PICHI_REQUIREMENTS_V1,
            "application/vnd.pichi.requirements.v1+yaml"
        );
        assert_eq!(
            MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD,
            "application/vnd.pichi.bootconfig.v1+zstd"
        );
        assert_eq!(
            MEDIA_TYPE_PICHI_CONFIG_V1,
            "application/vnd.pichi.config.v1+json"
        );
        assert_eq!(MEDIA_TYPE_OCI_EMPTY_V1, "application/vnd.oci.empty.v1+json");
    }

    #[test]
    fn all_constants_have_pichi_prefix_or_oci() {
        assert!(PichiArtifact::MEDIA_TYPE.starts_with("application/vnd.pichi."));
        assert!(ScuteDescriptor::MEDIA_TYPE.starts_with("application/vnd.pichi."));
        assert!(
            crate::artifact::Zstd::<crate::artifact::ScuteDescriptor>::MEDIA_TYPE
                .starts_with("application/vnd.pichi.")
        );
        assert!(
            crate::artifact::Zstd::<crate::artifact::ScuteDescriptor>::MEDIA_TYPE
                .ends_with("+zstd")
        );
        assert!(PmiDescriptor::MEDIA_TYPE.starts_with("application/vnd.pichi."));
        assert!(MEDIA_TYPE_PICHI_REQUIREMENTS_V1.starts_with("application/vnd.pichi."));
        assert!(MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD.starts_with("application/vnd.pichi."));
        assert!(MEDIA_TYPE_PICHI_CONFIG_V1.starts_with("application/vnd.pichi."));
        assert!(MEDIA_TYPE_PICHI_CONFIG_V1.ends_with("+json"));
        assert!(MEDIA_TYPE_OCI_EMPTY_V1.starts_with("application/vnd.oci."));
    }

    #[test]
    fn scute_v1_zstd_is_scute_v1_with_zstd_suffix() {
        // Discriminator pattern from D-08: variants share a stem.
        assert_eq!(
            format!("{}+zstd", ScuteDescriptor::MEDIA_TYPE),
            crate::artifact::Zstd::<crate::artifact::ScuteDescriptor>::MEDIA_TYPE
        );
    }
}
