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
