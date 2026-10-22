// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    cache::{Cache, ResolvedImage},
    scute::{Chain, Scute},
    snapshot::Snapshot,
};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    os::unix::fs::FileExt,
};

fn manifest(scutes: &[Scute]) -> Manifest {
    let mut annotations = BTreeMap::new();
    for (key, value) in [
        ("algo", "sha256"),
        ("data-block-size", "4096"),
        ("hash-block-size", "4096"),
        ("version", "1"),
        ("hash-type", "1"),
    ] {
        annotations.insert(format!("dev.pichi.carapace.verity.{key}"), value.into());
    }
    annotations.insert(
        "dev.pichi.carapace.verity.hash".into(),
        hex::encode(scutes.last().unwrap().root),
    );
    Manifest {
        schema_version: 2,
        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
        artifact_type: "application/vnd.pichi.artifact.v1+json".into(),
        config: ConfigDescriptor::canonical(),
        // Manifest order is not the chain order.
        layers: scutes
            .iter()
            .rev()
            .map(|s| Layer::Scute(s.descriptor.clone()))
            .collect(),
        annotations,
    }
}

#[test]
fn imported_scutes_reconstruct_the_authenticated_root() {
    let directory = tempfile::tempdir().unwrap();
    let raw = directory.path().join("raw");
    fs::write(&raw, [0x53; 8192]).unwrap();
    let base = Scute::import(&raw, &directory.path().join("base")).unwrap();
    let manifest = manifest(std::slice::from_ref(&base));
    let cache_dir = directory.path().join("cache");
    fs::create_dir_all(cache_dir.join("blobs/sha256")).unwrap();
    fs::copy(
        base.data_path(),
        cache_dir
            .join("blobs/sha256")
            .join(&base.descriptor.digest[7..]),
    )
    .unwrap();
    let cache = Cache::new(cache_dir);
    let mut image = ResolvedImage {
        manifest_bytes: manifest.to_bytes().unwrap(),
        manifest,
        index: String::new(),
        manifest_digest: String::new(),
    };
    let chain = Chain::stage(&cache, &image, &directory.path().join("staged")).unwrap();
    assert_eq!(chain.root().unwrap(), base.root);
    image
        .manifest
        .annotations
        .insert("dev.pichi.carapace.verity.hash".into(), "ff".repeat(32));
    assert!(Chain::stage(&cache, &image, &directory.path().join("bad")).is_err());
}

#[test]
#[ignore = "requires root, device-mapper, loop devices and tmpfs"]
fn kernel_snapshot_changes_become_a_verified_layer() {
    let directory = conglobate::mounts::Scratch::new().unwrap();
    let raw = directory.path().join("raw");
    fs::write(&raw, [0x53; 8192]).unwrap();
    let base = Scute::import(&raw, &directory.path().join("base")).unwrap();
    let mut chain = Chain { scutes: vec![base] };
    let origin = chain.assemble().unwrap();
    let mut actual = [0; 8192];
    origin.read_exact_at(&mut actual, 0).unwrap();
    assert_eq!(actual, [0x53; 8192]);
    let snapshot = Snapshot::create(
        origin,
        chain.root().unwrap(),
        &directory.path().join("snapshot"),
    )
    .unwrap();
    let device = File::options()
        .read(true)
        .write(true)
        .open(snapshot.device_path())
        .unwrap();
    device.write_all_at(&[0x77; 4096], 4096).unwrap();
    device.sync_all().unwrap();
    drop(device);
    let layer = snapshot.finish().unwrap();
    assert!(layer.descriptor.size < 64 * 1024);
    chain.scutes.push(layer);
    let result = chain.assemble().unwrap();
    result.read_exact_at(&mut actual, 0).unwrap();
    assert_eq!(&actual[..4096], &[0x53; 4096]);
    assert_eq!(&actual[4096..], &[0x77; 4096]);
    drop(result);
    // Unordered OCI layers can be independently reconstructed and verified.
    let manifest = manifest(&chain.scutes);
    let cache_dir = directory.path().join("cache");
    fs::create_dir_all(cache_dir.join("blobs/sha256")).unwrap();
    for scute in &chain.scutes {
        fs::copy(
            scute.data_path(),
            cache_dir
                .join("blobs/sha256")
                .join(&scute.descriptor.digest[7..]),
        )
        .unwrap();
    }
    let image = ResolvedImage {
        manifest_bytes: manifest.to_bytes().unwrap(),
        manifest,
        index: String::new(),
        manifest_digest: String::new(),
    };
    let staged = Chain::stage(
        &Cache::new(cache_dir),
        &image,
        &directory.path().join("staged"),
    )
    .unwrap();
    assert_eq!(staged.root().unwrap(), chain.root().unwrap());
}

#[test]
#[ignore = "requires root, device-mapper and private mount namespace"]
fn startup_recovers_an_interrupted_device_activation() {
    use std::os::unix::fs::PermissionsExt;
    let stale = tempfile::Builder::new()
        .prefix("conglobate-")
        .permissions(fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap();
    let lease = stale
        .path()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("conglobate-")
        .unwrap();
    let name = format!("conglobate-0-{lease}-0");
    let control = devmap_linux::Control::open().unwrap();
    let id = control.create(&name).unwrap();
    let path = stale.keep();
    // No table, open device handle, or deferred removal: simulate a supervisor
    // killed immediately after allocating the mapping.
    let recovered = conglobate::mounts::Scratch::new();
    let remains = control.by_name(&name).is_ok();
    if remains {
        control.remove(id, true).unwrap();
    }
    recovered.unwrap();
    assert!(!remains, "abandoned mapping survived startup recovery");
    assert!(!path.exists());
}

#[test]
fn planned_import_captures_exact_bytes_and_full_metadata_terminator() {
    let directory = tempfile::tempdir().unwrap();
    for blocks in [0, 1, 255, 256, 257] {
        let raw = directory.path().join(format!("raw-{blocks}"));
        let file = File::create(&raw).unwrap();
        file.set_len(16 * 1024 * 1024 * 1024).unwrap();
        for index in 0..blocks {
            file.write_all_at(&[0x51; 4096], (index + 7) * 4096)
                .unwrap();
        }
        // An allocated zero block is not an exception either.
        file.write_all_at(&[0; 4096], 8192).unwrap();
        drop(file);
        let base = Scute::import(&raw, &directory.path().join(format!("base-{blocks}"))).unwrap();
        let (digest, size) = conglobate::scute::digest_file(base.data_path()).unwrap();
        assert_eq!(base.descriptor.digest, digest);
        assert_eq!(base.descriptor.size, size);
        assert_eq!(size, (blocks + blocks / 256 + 2) * 4096);
        let resealed = Scute::seal(
            base.data_path().into(),
            directory.path().join(format!("hash-{blocks}")),
            [0; 32],
        )
        .unwrap();
        assert_eq!(resealed.root, base.root);
    }
}

#[test]
fn staging_preserves_partial_block_padding_and_rejects_wrong_extents() {
    use sha2::{Digest as _, Sha256};
    let directory = tempfile::tempdir().unwrap();
    let raw = directory.path().join("raw");
    fs::write(&raw, [0; 4096]).unwrap();
    let base = Scute::import(&raw, &directory.path().join("base")).unwrap();
    let complete = fs::read(base.data_path()).unwrap();
    assert_eq!(complete.len(), 8192);
    let cache_dir = directory.path().join("cache");
    fs::create_dir_all(cache_dir.join("blobs/sha256")).unwrap();
    let cache = Cache::new(cache_dir.clone());
    for compressed in [false, true] {
        for (index, (length, blocks, valid)) in [
            (8192, Some(2), true),
            (8191, Some(2), true),
            (8191, None, true),
            (8193, Some(2), false),
            (4096, Some(2), false),
            (8191, Some(3), false),
            (8191, Some(0), false),
            (8191, Some(u64::MAX), false),
            (0, None, false),
        ]
        .into_iter()
        .enumerate()
        {
            let mut bytes = complete.clone();
            bytes.resize(length, 0);
            let blob = if compressed {
                zstd::stream::encode_all(bytes.as_slice(), 1).unwrap()
            } else {
                bytes
            };
            let digest = format!("sha256:{:x}", Sha256::digest(&blob));
            fs::write(cache_dir.join("blobs/sha256").join(&digest[7..]), &blob).unwrap();
            let mut descriptor = base.descriptor.clone();
            descriptor.digest = digest;
            descriptor.size = blob.len() as u64;
            descriptor.annotations.data_blocks = blocks;
            let mut manifest = manifest(std::slice::from_ref(&base));
            manifest.layers = vec![if compressed {
                Layer::ScuteZstd(descriptor)
            } else {
                Layer::Scute(descriptor)
            }];
            let image = ResolvedImage {
                manifest_bytes: manifest.to_bytes().unwrap(),
                manifest,
                index: String::new(),
                manifest_digest: String::new(),
            };
            let result = Chain::stage(
                &cache,
                &image,
                &directory.path().join(format!("stage-{compressed}-{index}")),
            );
            assert_eq!(
                result.is_ok(),
                valid,
                "compressed={compressed}, length={length}, blocks={blocks:?}: {result:?}"
            );
            if valid {
                assert_eq!(result.unwrap().root().unwrap(), base.root);
            }
        }
    }
}

#[test]
fn authenticated_chain_rejects_disconnected_duplicate_and_missing_ancestry() {
    let directory = tempfile::tempdir().unwrap();
    let raw = directory.path().join("raw");
    fs::write(&raw, [0x53; 8192]).unwrap();
    let base = Scute::import(&raw, &directory.path().join("base")).unwrap();
    // Distinct parent salts bind an ordered ancestry even when layer bytes
    // happen to be identical. No kernel mappings are needed to validate it.
    let second = Scute::seal(
        base.data_path().into(),
        directory.path().join("second-hash"),
        base.root,
    )
    .unwrap();
    let third = Scute::seal(
        base.data_path().into(),
        directory.path().join("third-hash"),
        second.root,
    )
    .unwrap();
    let scutes = [base, second, third];
    let cache_dir = directory.path().join("cache");
    fs::create_dir_all(cache_dir.join("blobs/sha256")).unwrap();
    for scute in &scutes {
        fs::copy(
            scute.data_path(),
            cache_dir
                .join("blobs/sha256")
                .join(&scute.descriptor.digest[7..]),
        )
        .unwrap();
    }
    let cache = Cache::new(cache_dir.clone());
    let complete = manifest(&scutes);
    let stage = |manifest: Manifest, name: &str| {
        let image = ResolvedImage {
            manifest_bytes: manifest.to_bytes().unwrap(),
            manifest,
            index: String::new(),
            manifest_digest: String::new(),
        };
        Chain::stage(&cache, &image, &directory.path().join(name))
    };
    let valid = stage(complete.clone(), "valid").unwrap();
    assert_eq!(valid.scutes.len(), 3);
    for (actual, expected) in valid.scutes.iter().zip(&scutes) {
        assert_eq!(actual.root, expected.root);
        assert_eq!(actual.parent, expected.parent);
    }
    let mut missing = complete.clone();
    missing.layers.remove(1);
    assert!(
        stage(missing, "missing")
            .unwrap_err()
            .to_string()
            .contains("authenticated root")
    );
    let mut duplicate = complete.clone();
    duplicate.layers.push(duplicate.layers[0].clone());
    assert!(
        stage(duplicate, "duplicate")
            .unwrap_err()
            .to_string()
            .contains("duplicate scute roots")
    );
    // The same content/root encoded in a second transport is still a duplicate.
    let compressed =
        zstd::stream::encode_all(fs::read(scutes[2].data_path()).unwrap().as_slice(), 1).unwrap();
    let mut descriptor = scutes[2].descriptor.clone();
    descriptor.digest = conglobate::inventory::Digest::sha256(&compressed).oci();
    descriptor.size = compressed.len() as u64;
    fs::write(
        cache_dir.join("blobs/sha256").join(&descriptor.digest[7..]),
        compressed,
    )
    .unwrap();
    let mut duplicate_transport = complete.clone();
    duplicate_transport
        .layers
        .push(Layer::ScuteZstd(descriptor));
    assert!(
        stage(duplicate_transport, "duplicate-transport")
            .unwrap_err()
            .to_string()
            .contains("duplicate scute roots")
    );
    // All blobs are genuine, but a root selecting only the base leaves two
    // disconnected layers; authentic hashes alone cannot make that graph valid.
    let mut disconnected = complete.clone();
    disconnected.annotations.insert(
        "dev.pichi.carapace.verity.hash".into(),
        hex::encode(scutes[0].root),
    );
    assert!(
        stage(disconnected, "disconnected")
            .unwrap_err()
            .to_string()
            .contains("disconnected scutes")
    );
    let mut zero_root = complete.clone();
    zero_root
        .annotations
        .insert("dev.pichi.carapace.verity.hash".into(), "00".repeat(32));
    assert!(
        stage(zero_root, "zero-root")
            .unwrap_err()
            .to_string()
            .contains("disconnected scutes")
    );
    let mut no_layers = complete;
    no_layers.layers.clear();
    assert!(
        stage(no_layers, "no-layers")
            .unwrap_err()
            .to_string()
            .contains("authenticated root")
    );
}

#[test]
fn compressed_scutes_validate_complete_streams_and_decoded_root() {
    use std::io::Write as _;
    let directory = tempfile::tempdir().unwrap();
    let raw = directory.path().join("raw");
    fs::write(&raw, [0x53; 4096]).unwrap();
    let base = Scute::import(&raw, &directory.path().join("base")).unwrap();
    let bytes = fs::read(base.data_path()).unwrap();
    let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 1).unwrap();
    encoder.include_checksum(true).unwrap();
    encoder.write_all(&bytes).unwrap();
    let frame = encoder.finish().unwrap();
    let mut bad_checksum = frame.clone();
    *bad_checksum.last_mut().unwrap() ^= 1;
    let mut junk_suffix = frame.clone();
    junk_suffix.extend_from_slice(b"not a zstd frame");
    let mut appended_data = frame.clone();
    appended_data.extend(zstd::stream::encode_all(&[0x55; 4096][..], 1).unwrap());
    let midpoint = bytes.len() / 2;
    let mut split_frames = zstd::stream::encode_all(&bytes[..midpoint], 1).unwrap();
    split_frames.extend(zstd::stream::encode_all(&bytes[midpoint..], 1).unwrap());
    let cases = [
        ("valid", frame.clone(), "valid"),
        ("split-frames", split_frames, "valid"),
        ("not-zstd", b"not a zstd frame".to_vec(), "decoder"),
        ("short-header", frame[..3].to_vec(), "decoder"),
        (
            "truncated-checksum",
            frame[..frame.len() - 1].to_vec(),
            "decoder",
        ),
        ("checksum-mismatch", bad_checksum, "decoder"),
        ("junk-suffix", junk_suffix, "decoder"),
        ("appended-data", appended_data, "root"),
    ];
    let cache_dir = directory.path().join("cache");
    fs::create_dir_all(cache_dir.join("blobs/sha256")).unwrap();
    let cache = Cache::new(cache_dir.clone());
    for (name, blob, outcome) in cases {
        // Every blob, including invalid streams, has a matching OCI digest and
        // size. Rejection must come from decoding or authenticated decoded data.
        let digest = conglobate::inventory::Digest::sha256(&blob).oci();
        fs::write(cache_dir.join("blobs/sha256").join(&digest[7..]), &blob).unwrap();
        let mut descriptor = base.descriptor.clone();
        descriptor.digest = digest;
        descriptor.size = blob.len() as u64;
        descriptor.annotations.data_blocks = None;
        let mut manifest = manifest(std::slice::from_ref(&base));
        manifest.layers = vec![Layer::ScuteZstd(descriptor)];
        let image = ResolvedImage {
            manifest_bytes: manifest.to_bytes().unwrap(),
            manifest,
            index: String::new(),
            manifest_digest: String::new(),
        };
        let staging = directory.path().join(name);
        let result = Chain::stage(&cache, &image, &staging);
        assert_eq!(fs::read(staging.join("0/source")).unwrap(), blob);
        match outcome {
            "valid" => {
                let chain = result.unwrap();
                assert_eq!(chain.root().unwrap(), base.root);
                assert_eq!(fs::read(chain.scutes[0].data_path()).unwrap(), bytes);
            }
            "decoder" => assert!(
                matches!(result, Err(conglobate::Error::Io(_))),
                "{name}: {result:?}"
            ),
            "root" => assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("authenticated root"),
                "{name}"
            ),
            _ => unreachable!(),
        }
    }
}
