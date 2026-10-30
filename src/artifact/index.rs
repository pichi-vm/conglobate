// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MEDIA_TYPE_OCI_INDEX_V1: &str = "application/vnd.oci.image.index.v1+json";
pub const MEDIA_TYPE_OCI_MANIFEST_V1: &str = "application/vnd.oci.image.manifest.v1+json";

/// OCI index exchanged by the builder, registry, and local image store.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub schema_version: u32,
    pub media_type: String,
    pub manifests: Vec<IndexDescriptor>,
}

impl Default for Index {
    fn default() -> Self {
        Self {
            schema_version: 2,
            media_type: MEDIA_TYPE_OCI_INDEX_V1.into(),
            manifests: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IndexDescriptor {
    pub media_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
    pub digest: String,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Platform {
    pub os: String,
    pub architecture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutMarker {
    pub image_layout_version: String,
}

impl Default for LayoutMarker {
    fn default() -> Self {
        Self {
            image_layout_version: "1.0.0".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_builder_platform_and_local_tag_annotations() {
        let text = r#"{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{"mediaType":"application/vnd.oci.image.manifest.v1+json","artifactType":"application/vnd.pichi.artifact.v1+json","digest":"sha256:abcd","size":123,"platform":{"os":"pichi","architecture":"arm64","variant":"v8"},"annotations":{"org.opencontainers.image.ref.name":"example:test"}}]}"#;
        let index: Index = serde_json::from_str(text).unwrap();
        assert_eq!(serde_json::to_string(&index).unwrap(), text);
    }

    #[test]
    fn minimal_descriptor_does_not_gain_optional_fields() {
        let text = r#"{"mediaType":"application/vnd.oci.image.manifest.v1+json","digest":"sha256:abcd","size":123}"#;
        let descriptor: IndexDescriptor = serde_json::from_str(text).unwrap();
        assert_eq!(serde_json::to_string(&descriptor).unwrap(), text);
        assert!(serde_json::from_str::<Index>(r#"{"schemaVersion":2,"mediaType":"x"}"#).is_err());
    }
}
