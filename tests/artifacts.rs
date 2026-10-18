// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    artifact::Artifact,
    inventory::{self as inv, Digest, Inventory},
};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest, PmiDescriptor};
use std::{collections::BTreeMap, fs};

fn fixture(manifest: &Manifest) -> Inventory {
    let digest = Digest::sha256(b"fixture");
    Inventory {
        version: 1,
        builder: inv::Builder {
            name: "conglobate".into(),
            version: "test".into(),
            executable: digest.clone(),
        },
        context: inv::Context {
            repository: "/source".into(),
            object_format: "sha1".into(),
            commit: vec![1; 20],
            subdirectory: ".".into(),
            tree: vec![2; 20],
        },
        inputs: vec![],
        execution: inv::Execution {
            architecture: "amd64".into(),
            recipes: BTreeMap::from([
                ("firmware.yaml".into(), digest.clone()),
                ("refs.lock".into(), digest.clone()),
            ]),
            transform: inv::Transform {
                source: "builtin".into(),
                digest,
                target_present: false,
                default_skipped: true,
            },
            network_steps: vec![],
        },
        outputs: inv::Outputs {
            manifest: Digest::sha256(&manifest.to_bytes().unwrap()),
            carapace_root: None,
            scutes: vec![],
            pmi: Some(Digest::from_oci(manifest.layers[0].digest_str()).unwrap()),
            dtb: None,
        },
    }
}

fn manifest() -> Manifest {
    Manifest {
        schema_version: 2,
        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
        artifact_type: "application/vnd.pichi.artifact.v1+json".into(),
        config: ConfigDescriptor::canonical(),
        layers: vec![Layer::Pmi(PmiDescriptor {
            digest: Digest::sha256(b"firmware").oci(),
            size: 8,
        })],
        annotations: BTreeMap::new(),
    }
}

#[test]
fn deterministic_inventory_roundtrip_and_strict_decoding() {
    let inventory = fixture(&manifest());
    let bytes = inventory.encode().unwrap();
    assert_eq!(Inventory::decode(&bytes).unwrap().encode().unwrap(), bytes);
    let value: ciborium::Value = ciborium::from_reader(bytes.as_slice()).unwrap();
    let mut reordered = value.clone();
    let ciborium::Value::Map(entries) = &mut reordered else {
        panic!()
    };
    entries.reverse();
    let mut encoded = Vec::new();
    ciborium::into_writer(&reordered, &mut encoded).unwrap();
    assert!(Inventory::decode(&encoded).is_err());
    let mut appended = bytes;
    appended.push(0);
    assert!(Inventory::decode(&appended).is_err());
    let mut invalid = fixture(&manifest());
    invalid.version = 2;
    assert!(invalid.encode().is_err());
    invalid.version = 1;
    invalid.outputs.manifest.digest.pop();
    assert!(invalid.encode().is_err());
    // SHA digests are native CBOR byte strings, never arrays of integers.
    let ciborium::Value::Map(top) = value else {
        panic!()
    };
    let (_, outputs) = top
        .iter()
        .find(|(key, _)| key.as_text() == Some("outputs"))
        .unwrap();
    let (_, manifest) = outputs
        .as_map()
        .unwrap()
        .iter()
        .find(|(key, _)| key.as_text() == Some("manifest"))
        .unwrap();
    let (_, digest) = manifest
        .as_map()
        .unwrap()
        .iter()
        .find(|(key, _)| key.as_text() == Some("digest"))
        .unwrap();
    assert!(matches!(digest, ciborium::Value::Bytes(bytes) if bytes.len() == 32));
}

#[test]
fn output_is_self_contained_and_existing_destinations_are_never_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let artifact = Artifact::new(&directory.path().join("private")).unwrap();
    let manifest = manifest();
    let inventory = fixture(&manifest);
    artifact.put(&b"firmware"[..]).unwrap();
    let (unused, _) = artifact.put(&b"old firmware"[..]).unwrap();
    artifact.finalize(&manifest, &inventory).unwrap();
    assert!(!artifact.blob_path(&unused.oci()).unwrap().exists());
    let output = directory.path().join("output");
    artifact.publish(&output).unwrap();
    let index: serde_json::Value =
        serde_json::from_slice(&fs::read(output.join("image.oci/index.json")).unwrap()).unwrap();
    assert_eq!(
        index["manifests"][0]["digest"],
        inventory.outputs.manifest.oci()
    );
    assert_eq!(index["manifests"][0]["platform"]["os"], "pichi");
    assert_eq!(
        index["manifests"][0]["artifactType"],
        pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1
    );
    for (digest, size) in [
        (manifest.config.digest.as_str(), manifest.config.size),
        (manifest.layers[0].digest_str(), 8),
    ] {
        let bytes = fs::read(output.join("image.oci/blobs/sha256").join(&digest[7..])).unwrap();
        assert_eq!(bytes.len() as u64, size);
        assert_eq!(Digest::sha256(&bytes).oci(), digest);
    }
    Inventory::decode(&fs::read(output.join("inventory.cbor")).unwrap()).unwrap();
    for name in ["output", "empty"] {
        let destination = directory.path().join(name);
        if name == "empty" {
            fs::create_dir(&destination).unwrap();
        }
        let artifact = Artifact::new(&directory.path().join(format!("private-{name}"))).unwrap();
        artifact.put(&b"firmware"[..]).unwrap();
        artifact.finalize(&manifest, &inventory).unwrap();
        assert!(artifact.publish(&destination).is_err());
        assert!(destination.is_dir());
    }
}

#[test]
fn unfinished_corrupt_or_mismatched_artifacts_cannot_publish() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("output");
    let artifact = Artifact::new(&directory.path().join("unfinished")).unwrap();
    assert!(artifact.publish(&output).is_err());
    assert!(!output.exists());
    let artifact = Artifact::new(&directory.path().join("corrupt")).unwrap();
    let manifest = manifest();
    let mut inventory = fixture(&manifest);
    assert!(artifact.finalize(&manifest, &inventory).is_err());
    artifact.put(&b"firmware"[..]).unwrap();
    inventory.outputs.pmi = Some(Digest::sha256(b"wrong"));
    assert!(artifact.finalize(&manifest, &inventory).is_err());
    inventory.outputs.pmi = Some(Digest::sha256(b"firmware"));
    fs::write(
        artifact.blob_path(manifest.layers[0].digest_str()).unwrap(),
        b"tampered",
    )
    .unwrap();
    assert!(artifact.finalize(&manifest, &inventory).is_err());
}

#[test]
fn firmware_only_input_omits_root_but_execution_inputs_require_one() {
    let mut inventory = fixture(&manifest());
    let digest = Digest::sha256(b"input");
    inventory.inputs.push(inv::Input {
        roles: vec!["carapace-base".into()],
        reference: "base".into(),
        index: digest.clone(),
        manifest: digest,
        carapace_root: None,
    });
    let bytes = inventory.encode().unwrap();
    assert!(
        Inventory::decode(&bytes).unwrap().inputs[0]
            .carapace_root
            .is_none()
    );
    let value: ciborium::Value = ciborium::from_reader(bytes.as_slice()).unwrap();
    let inputs = value
        .as_map()
        .unwrap()
        .iter()
        .find(|(k, _)| k.as_text() == Some("inputs"))
        .unwrap()
        .1
        .as_array()
        .unwrap();
    assert!(
        !inputs[0]
            .as_map()
            .unwrap()
            .iter()
            .any(|(k, _)| k.as_text() == Some("carapace_root"))
    );
    for role in ["bootstrap-tools", "firmware-tools"] {
        inventory.inputs[0].roles = vec![role.into()];
        assert!(inventory.encode().is_err());
    }
    inventory.inputs[0].carapace_root = Some(vec![0; 31].into());
    assert!(inventory.encode().is_err());
    inventory.inputs[0].carapace_root = Some(vec![0; 32].into());
    Inventory::decode(&inventory.encode().unwrap()).unwrap();
}
