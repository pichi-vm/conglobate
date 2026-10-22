//! Bounded authenticated carapace -> snapshot/FUSE -> derived chain probe.
#![forbid(unsafe_code)]
#[path = "../chain_publication.rs"]
mod publication;
use conglobate::{
    cache::{Cache, ResolvedImage},
    mounts::Scratch,
    scute::{Chain, Scute, digest_file},
    snapshot::Snapshot,
};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest};
use std::{
    collections::BTreeMap,
    error::Error,
    fs::{self, File},
    io::Read,
    os::unix::fs::{FileExt, chown},
    path::{Path, PathBuf},
    process::Command,
};

fn image(scutes: &[Scute]) -> Result<ResolvedImage, Box<dyn Error>> {
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
        hex::encode(scutes.last().ok_or("no scutes")?.root),
    );
    let manifest = Manifest {
        schema_version: 2,
        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
        artifact_type: "application/vnd.pichi.artifact.v1+json".into(),
        config: ConfigDescriptor::canonical(),
        // Exercise graph reconstruction rather than relying on OCI layer order.
        layers: scutes
            .iter()
            .rev()
            .map(|s| Layer::Scute(s.descriptor.clone()))
            .collect(),
        annotations,
    };
    Ok(ResolvedImage {
        manifest_bytes: manifest.to_bytes()?,
        manifest,
        index: String::new(),
        manifest_digest: String::new(),
    })
}

fn cache(scutes: &[Scute], directory: &Path) -> Result<Cache, Box<dyn Error>> {
    fs::create_dir_all(directory.join("blobs/sha256"))?;
    for scute in scutes {
        fs::copy(
            scute.data_path(),
            directory
                .join("blobs/sha256")
                .join(&scute.descriptor.digest[7..]),
        )?;
    }
    Ok(Cache::new(directory.to_owned()))
}

fn main() -> Result<(), Box<dyn Error>> {
    let scratch = Scratch::new()?;
    let base = Scute::import(Path::new("/work/base"), &scratch.path().join("base"))?;
    let mut bases = vec![base];
    if std::env::var_os("CHAIN_SHARED_BLOCK").is_some() {
        let data = scratch.path().join("shared-layer-data");
        fs::copy("/shared/layer/data", &data)?;
        let layer = Scute::seal(
            data,
            scratch.path().join("shared-layer-hash"),
            bases[0].root,
        )?;
        assert_eq!(hex::encode(layer.root), fs::read_to_string("/shared/root")?);
        bases.push(layer);
    }
    let base = &bases[0];
    let input_layers = bases.len();
    let base_digest = digest_file(base.data_path())?;
    let raw_digest = digest_file(Path::new("/work/base"))?;
    let mut base_image = image(&bases)?;
    let base_cache_path = scratch.path().join("cache");
    let base_cache = cache(&bases, &base_cache_path)?;
    let mut wrong = image(&bases)?;
    wrong
        .manifest
        .annotations
        .insert("dev.pichi.carapace.verity.hash".into(), "ff".repeat(32));
    assert!(Chain::stage(&base_cache, &wrong, &scratch.path().join("wrong-root")).is_err());
    println!("BASE_WRONG_ROOT_REJECTED");
    let blob = base_cache_path
        .join("blobs/sha256")
        .join(&base.descriptor.digest[7..]);
    let file = File::options().read(true).write(true).open(&blob)?;
    let mut old = [0];
    file.read_exact_at(&mut old, 4096)?;
    file.write_all_at(&[old[0] ^ 1], 4096)?;
    assert!(Chain::stage(&base_cache, &base_image, &scratch.path().join("corrupt")).is_err());
    file.write_all_at(&old, 4096)?;
    drop(file);
    println!("BASE_CORRUPT_BLOB_REJECTED");
    let inputs = if std::env::args().any(|arg| arg == "--publish") {
        let inputs = publication::prepare(&base_cache_path, &base_image)?;
        base_image = base_cache.resolve(
            &inputs
                .images
                .values()
                .next()
                .unwrap()
                .index
                .clone()
                .try_into()?,
        )?;
        Some(inputs)
    } else {
        None
    };
    let mut chain = Chain::stage(&base_cache, &base_image, &scratch.path().join("staged"))?;
    let rounds = if std::env::args().nth(1).as_deref() == Some("--four-layers") {
        3
    } else {
        1
    };
    for round in 1..=rounds {
        let prior_digests = chain
            .scutes
            .iter()
            .map(|s| digest_file(s.data_path()))
            .collect::<Result<Vec<_>, _>>()?;
        let parent = chain.root()?;
        // Trusted fixture-only alternative: both processes open the same
        // read-only data/hash block devices, independently assembling them.
        // Compare the supplied root to the already authenticated staged input.
        let origin = if std::env::var_os("CHAIN_SHARED_BLOCK").is_some() {
            use carapace::Assemble as _;
            assert_eq!(round, 1);
            assert_eq!(fs::read_to_string("/shared/root")?, hex::encode(parent));
            let scutes = ["base", "layer"]
                .into_iter()
                .map(|name| {
                    let data = fs::read_to_string(format!("/shared/{name}-data-device"))?;
                    let hash = fs::read_to_string(format!("/shared/{name}-hash-device"))?;
                    Ok(carapace::Scute {
                        data: File::open(data.trim())?,
                        hash: File::open(hash.trim())?,
                    })
                })
                .collect::<Result<Vec<_>, std::io::Error>>()?;
            let shared = carapace::Chain {
                root: parent.to_vec().into_boxed_slice(),
                scutes,
            };
            let device = shared.assemble()?;
            println!("SHARED_BLOCK_CHAIN_ASSEMBLED");
            device
        } else {
            chain.assemble()?
        };
        let snapshot = Snapshot::create(
            origin,
            parent,
            &scratch.path().join(format!("snapshot-{round}")),
        )?;
        chown(snapshot.device_path(), Some(100000), Some(100000))?;
        // Trusted validation-only launcher override permits cgroup placement
        // before dropping into the worker's user namespace.
        let sanitized = std::env::var_os("CHAIN_SANITIZE_LAUNCH").is_some();
        let launcher = if sanitized {
            assert!(std::env::var_os("CHAIN_WORKER_LAUNCHER").is_none());
            "/usr/bin/unshare".into()
        } else {
            std::env::var("CHAIN_WORKER_LAUNCHER").unwrap_or_else(|_| "unshare".into())
        };
        let mut command = Command::new(launcher);
        if sanitized {
            command.env_clear().envs([
                ("PATH", "/usr/bin:/usr/sbin"),
                ("HOME", "/"),
                ("LC_ALL", "C"),
            ]);
            println!("TRUSTED_WORKER_LAUNCH_ENV_CONSTRUCTED");
        }
        let status = command
            .args([
                "--user",
                "--mount",
                "--net",
                "--map-users",
                "0:100000:65536",
                "--map-groups",
                "0:100000:65536",
                "--setuid",
                "0",
                "--setgid",
                "0",
                "/bin/bash",
                "/chain-worker",
            ])
            .arg(snapshot.device_path())
            .status()?;
        assert!(status.success(), "worker failed: {status}");
        let layer = snapshot.finish()?;
        assert_eq!(layer.parent, parent);
        assert_ne!(layer.root, parent);
        println!("FUSE_DERIVED_LAYER_SEALED");
        for (scute, expected) in chain.scutes.iter().zip(prior_digests) {
            assert_eq!(digest_file(scute.data_path())?, expected);
        }
        chain.scutes.push(layer);
        println!("CHAIN_ROUND_SEALED {round}");
    }

    let derived_image = image(&chain.scutes)?;
    let derived_cache = cache(&chain.scutes, &scratch.path().join("derived-cache"))?;
    if rounds == 3 {
        let mut missing = image(&chain.scutes)?;
        missing.manifest.layers.remove(1);
        let error = Chain::stage(
            &derived_cache,
            &missing,
            &scratch.path().join("missing-middle"),
        )
        .err()
        .ok_or("missing ancestor accepted")?;
        assert!(
            error
                .to_string()
                .contains("scute chain does not match the authenticated root"),
            "{error}"
        );
        println!("MISSING_MIDDLE_LAYER_REJECTED {error}");
        let mut duplicate = image(&chain.scutes)?;
        duplicate
            .manifest
            .layers
            .push(duplicate.manifest.layers[1].clone());
        let error = Chain::stage(
            &derived_cache,
            &duplicate,
            &scratch.path().join("duplicate-middle"),
        )
        .err()
        .ok_or("duplicate ancestor accepted")?;
        assert!(
            error.to_string().contains("duplicate scute roots"),
            "{error}"
        );
        println!("DUPLICATE_MIDDLE_LAYER_REJECTED {error}");
    }
    let restaged = Chain::stage(
        &derived_cache,
        &derived_image,
        &scratch.path().join("derived-staged"),
    )?;
    assert_eq!(restaged.root()?, chain.root()?);
    assert_eq!(restaged.scutes.len(), rounds + input_layers);
    for (actual, expected) in restaged.scutes.iter().zip(&chain.scutes) {
        assert_eq!(actual.root, expected.root);
        assert_eq!(actual.parent, expected.parent);
    }
    println!("DERIVED_CHAIN_RESTAGED");
    let verified = PathBuf::from("/work/verified");
    // Carapace's logical volume is larger; only the fixture filesystem extent
    // is copied. Reading it exercises dm-verity on the reconstructed chain.
    let count = std::io::copy(
        &mut restaged.assemble()?.take(64 * 1024 * 1024),
        &mut File::create(&verified)?,
    )?;
    assert_eq!(count, 64 * 1024 * 1024);
    assert!(
        Command::new("/bin/bash")
            .arg("/verify")
            .arg(&verified)
            .status()?
            .success()
    );
    // Trusted diagnostic injection into this private disposable copy, after
    // userspace staging verification. Do not update its saved verity tree.
    let changed = File::options().read(true).write(true).open(
        restaged
            .scutes
            .last()
            .ok_or("no derived layer")?
            .data_path(),
    )?;
    let mut original = [0];
    changed.read_exact_at(&mut original, 8192)?;
    changed.write_all_at(&[original[0] ^ 1], 8192)?;
    changed.sync_all()?;
    let corrupted_read = match restaged.assemble() {
        Ok(device) => std::io::copy(&mut device.take(64 * 1024 * 1024), &mut std::io::sink())
            .map(|_| ())
            .map_err(conglobate::Error::Io),
        Err(error) => Err(error),
    };
    changed.write_all_at(&original, 8192)?;
    changed.sync_all()?;
    assert!(
        matches!(&corrupted_read, Err(conglobate::Error::Io(error)) if error.raw_os_error() == Some(5)),
        "expected EIO for post-staging corruption: {corrupted_read:?}"
    );
    println!("POST_STAGE_CORRUPTION_REJECTED {corrupted_read:?}");
    let restored = std::io::copy(
        &mut restaged.assemble()?.take(64 * 1024 * 1024),
        &mut std::io::sink(),
    )?;
    assert_eq!(restored, 64 * 1024 * 1024);
    println!("POST_STAGE_RESTORATION_READ_PASS");
    assert_eq!(digest_file(base.data_path())?, base_digest);
    assert_eq!(digest_file(Path::new("/work/base"))?, raw_digest);
    println!("BASE_SCUTE_UNCHANGED_PASS");
    if let Some(inputs) = &inputs {
        publication::publish(inputs, &chain, &derived_image, scratch.path(), input_layers)?;
    }
    Ok(())
}
