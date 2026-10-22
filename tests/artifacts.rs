// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    artifact::Artifact,
    inventory::{self as inv, Digest, Inventory},
};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest, PmiDescriptor};
use std::{collections::BTreeMap, fs};

#[test]
fn export_streams_with_bounded_reads_and_rejects_partial_io_failure() {
    use std::io::{self, Read};
    struct FixtureReader {
        remaining: usize,
        fail_at_end: bool,
        largest_request: usize,
    }
    impl Read for FixtureReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            self.largest_request = self.largest_request.max(buffer.len());
            if self.remaining == 0 && self.fail_at_end {
                return Err(io::Error::from_raw_os_error(nix::libc::EIO));
            }
            let count = buffer.len().min(self.remaining);
            buffer[..count].fill(0x5a);
            self.remaining -= count;
            Ok(count)
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let good = Artifact::new(&directory.path().join("good")).unwrap();
    let mut reader = FixtureReader {
        remaining: 4 * 1024 * 1024,
        fail_at_end: false,
        largest_request: 0,
    };
    let (digest, size) = good.put(&mut reader).unwrap();
    assert_eq!(size, 4 * 1024 * 1024);
    assert!(reader.largest_request <= 65536);
    assert_eq!(
        fs::metadata(good.blob_path(&digest.oci()).unwrap())
            .unwrap()
            .len(),
        size
    );

    let failed_path = directory.path().join("failed");
    let failed = Artifact::new(&failed_path).unwrap();
    let mut reader = FixtureReader {
        remaining: 3 * 65536,
        fail_at_end: true,
        largest_request: 0,
    };
    let error = failed.put(&mut reader).unwrap_err();
    assert!(
        matches!(error, conglobate::Error::Io(error) if error.raw_os_error() == Some(nix::libc::EIO))
    );
    assert_eq!(
        fs::read_dir(failed_path.join("image.oci/blobs/sha256"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(fs::read_dir(&failed_path).unwrap().count(), 1);
    let destination = directory.path().join("output");
    assert!(failed.publish(&destination).is_err());
    assert!(!destination.exists());
}

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

#[test]
fn artifact_stream_cancellation_is_isolated_and_cleans_staging() {
    struct CancelReader {
        calls: usize,
        eof: bool,
        cancel_at: usize,
    }
    impl std::io::Read for CancelReader {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.calls += 1;
            if self.calls < self.cancel_at {
                buffer.fill(0x5a);
                return Ok(buffer.len());
            }
            if self.calls == self.cancel_at {
                nix::sys::signal::raise(nix::sys::signal::Signal::SIGTERM).unwrap();
                if !self.eof {
                    buffer.fill(0x5a);
                    return Ok(buffer.len());
                }
            }
            Ok(0)
        }
    }
    const MODE: &str = "CONGLOBATE_ARTIFACT_CANCEL_TEST";
    let Ok(mode) = std::env::var(MODE) else {
        // The cancellation flag is process-global and intentionally irreversible.
        for mode in ["before-read", "during-read", "after-chunk", "at-eof"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "artifact_stream_cancellation_is_isolated_and_cleans_staging",
                    "--nocapture",
                ])
                .env(MODE, mode)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{mode}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    conglobate::cancel::install().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let private = directory.path().join("private");
    let artifact = Artifact::new(&private).unwrap();
    let mut reader = CancelReader {
        calls: 0,
        eof: mode == "at-eof",
        cancel_at: if mode == "after-chunk" { 2 } else { 1 },
    };
    if mode == "before-read" {
        nix::sys::signal::raise(nix::sys::signal::Signal::SIGTERM).unwrap();
    }
    let error = artifact.put(&mut reader).unwrap_err();
    assert_eq!(error.to_string(), "build cancelled");
    assert_eq!(
        reader.calls,
        match mode.as_str() {
            "before-read" => 0,
            "after-chunk" => 2,
            _ => 1,
        }
    );
    assert_eq!(
        fs::read_dir(private.join("image.oci/blobs/sha256"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(fs::read_dir(&private).unwrap().count(), 1);
}

#[test]
fn cancellation_during_publication_copy_removes_sibling_staging() {
    const SIZE: u64 = 128 * 1024 * 1024;
    const CHILD: &str = "CONGLOBATE_PUBLICATION_CANCEL_TEST";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "cancellation_during_publication_copy_removes_sibling_staging",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    conglobate::cancel::install().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let private = directory.path().join("private");
    let artifact = Artifact::new(&private).unwrap();
    let (digest, size) = artifact
        .put(std::io::Read::take(std::io::repeat(0x5a), SIZE))
        .unwrap();
    let mut manifest = manifest();
    manifest.layers = vec![Layer::Pmi(PmiDescriptor {
        digest: digest.oci(),
        size,
    })];
    artifact.finalize(&manifest, &fixture(&manifest)).unwrap();
    let root = directory.path().to_owned();
    let blob = digest.oci()[7..].to_owned();
    let observer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            for entry in fs::read_dir(&root).unwrap().flatten() {
                if !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".conglobate-output-")
                {
                    continue;
                }
                let path = entry.path().join("image.oci/blobs/sha256").join(&blob);
                if let Ok(metadata) = fs::metadata(path)
                    && metadata.len() > 0
                    && metadata.len() < SIZE
                {
                    // The production publication copy is active. The handler
                    // is process-wide even though this signal targets this thread.
                    nix::sys::signal::raise(nix::sys::signal::Signal::SIGTERM).unwrap();
                    return metadata.len();
                }
            }
            std::thread::yield_now();
        }
        panic!("did not observe a partially copied publication blob");
    });
    let output = directory.path().join("published");
    let result = artifact.publish(&output);
    let observed = observer.join().unwrap();
    assert!(observed > 0 && observed < SIZE);
    assert_eq!(result.unwrap_err().to_string(), "build cancelled");
    assert!(!output.exists());
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    assert!(private.join("inventory.cbor").is_file());
    println!(
        "cancelled during publication: observed {observed}/{SIZE} copied bytes; no output or sibling staging remains"
    );
}

#[test]
fn publication_race_preserves_a_new_empty_destination() {
    const SIZE: u64 = 32 * 1024 * 1024;
    use std::os::unix::fs::MetadataExt;
    let directory = tempfile::tempdir().unwrap();
    let private = directory.path().join("private");
    let artifact = Artifact::new(&private).unwrap();
    let (digest, size) = artifact
        .put(std::io::Read::take(std::io::repeat(0x5a), SIZE))
        .unwrap();
    let mut manifest = manifest();
    manifest.layers = vec![Layer::Pmi(PmiDescriptor {
        digest: digest.oci(),
        size,
    })];
    artifact.finalize(&manifest, &fixture(&manifest)).unwrap();
    let output = directory.path().join("published");
    let competing_output = output.clone();
    let root = directory.path().to_owned();
    let blob = digest.oci()[7..].to_owned();
    let observer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while std::time::Instant::now() < deadline {
            for entry in fs::read_dir(&root).unwrap().flatten() {
                if !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".conglobate-output-")
                {
                    continue;
                }
                let path = entry.path().join("image.oci/blobs/sha256").join(&blob);
                if let Ok(metadata) = fs::metadata(path)
                    && metadata.len() > 0
                    && metadata.len() < SIZE
                {
                    // A separate process creates an empty directory after the
                    // early existence check. Plain rename could replace it.
                    assert!(
                        std::process::Command::new("mkdir")
                            .arg(&competing_output)
                            .status()
                            .unwrap()
                            .success()
                    );
                    let created = fs::metadata(&competing_output).unwrap();
                    return (created.dev(), created.ino(), metadata.len());
                }
            }
            std::thread::yield_now();
        }
        panic!("did not observe a partially copied publication blob");
    });
    let result = artifact.publish(&output);
    let (dev, ino, observed) = observer.join().unwrap();
    assert!(
        matches!(result, Err(conglobate::Error::Io(ref error)) if error.raw_os_error() == Some(nix::libc::EEXIST)),
        "{result:?}"
    );
    let preserved = fs::metadata(&output).unwrap();
    assert_eq!((preserved.dev(), preserved.ino()), (dev, ino));
    assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
    assert!(private.join("inventory.cbor").is_file());
    println!(
        "destination created during copy at {observed}/{SIZE} bytes: EEXIST; empty directory inode preserved; sibling staging removed"
    );
}
