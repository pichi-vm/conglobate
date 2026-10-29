// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Round-trip tests for a bootable artifact manifest, using only the public
//! library API, the same way Pichi uses it.

use std::collections::BTreeMap;

use conglobate::artifact::{
    ANNOTATION_CARAPACE_VERITY_ALGO, ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE,
    ANNOTATION_CARAPACE_VERITY_HASH, ANNOTATION_CARAPACE_VERITY_HASH_BLOCK_SIZE,
    ANNOTATION_CARAPACE_VERITY_HASH_TYPE, ANNOTATION_CARAPACE_VERITY_VERSION, ConfigDescriptor,
    Layer, MEDIA_TYPE_OCI_IMAGE_MANIFEST_V1, MEDIA_TYPE_PICHI_ARTIFACT_V1, Manifest, PmiDescriptor,
    ScuteAnnotations, ScuteDescriptor,
};

fn chain_annotations() -> BTreeMap<String, String> {
    [
        (ANNOTATION_CARAPACE_VERITY_ALGO, "sha256"),
        (ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE, "4096"),
        (ANNOTATION_CARAPACE_VERITY_HASH_BLOCK_SIZE, "4096"),
        (ANNOTATION_CARAPACE_VERITY_VERSION, "1"),
        (ANNOTATION_CARAPACE_VERITY_HASH_TYPE, "1"),
        // Older manifests may still carry the newest layer's root hash.
        // Readers ignore it, so it must not stop the manifest from loading.
        (
            ANNOTATION_CARAPACE_VERITY_HASH,
            "abababababababababababababababababababababababababababababababab",
        ),
        ("org.opencontainers.image.created", "2026-05-06T14:32:00Z"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect()
}

/// A bootable artifact: one filesystem layer plus a boot image (PMI).
fn appliance_manifest() -> Manifest {
    Manifest {
        schema_version: 2,
        media_type: MEDIA_TYPE_OCI_IMAGE_MANIFEST_V1.into(),
        artifact_type: MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
        config: ConfigDescriptor::canonical(),
        layers: vec![
            Layer::Scute(ScuteDescriptor {
                digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .into(),
                size: 4096,
                annotations: ScuteAnnotations {
                    data_blocks: None,
                    salt: "dead".into(),
                },
            }),
            Layer::Pmi(PmiDescriptor {
                digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                    .into(),
                size: 8192,
            }),
        ],
        annotations: chain_annotations(),
    }
}

#[test]
fn appliance_round_trip_and_validates() {
    let manifest = appliance_manifest();
    let bytes = manifest.to_bytes().expect("serialize");
    let parsed = Manifest::from_reader_validated(bytes.as_slice()).expect("parse and validate");
    assert_eq!(manifest, parsed);
}

#[test]
fn artifact_type_lives_at_top_level_not_in_config() {
    let bytes = appliance_manifest().to_bytes().unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        value["artifactType"].as_str(),
        Some(MEDIA_TYPE_PICHI_ARTIFACT_V1)
    );
    assert_ne!(
        value["config"]["mediaType"].as_str(),
        Some(MEDIA_TYPE_PICHI_ARTIFACT_V1)
    );
}
