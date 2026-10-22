//! Exercise real scute conversion, inventory encoding and OCI publication.
#![forbid(unsafe_code)]
use conglobate::{
    artifact::Artifact,
    inventory::{self as inv, Digest, Inventory},
    scute::Scute,
};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest};
use std::{collections::BTreeMap, error::Error, fs, path::PathBuf};

fn main() {
    if let Err(error) = run() {
        if matches!(error.downcast_ref::<conglobate::Error>(),
                    Some(conglobate::Error::Io(io)) if io.kind() == std::io::ErrorKind::StorageFull)
        {
            eprintln!("ARTIFACT_ENOSPC_CONFIRMED");
            std::process::exit(28);
        }
        eprintln!("{error:?}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    assert_eq!(args.len(), 4);
    let before = conglobate::scute::digest_file(&args[0])?;
    let scute = Scute::import(&args[0], &args[1])?;
    let artifact = Artifact::new(&args[2])?;
    let (digest, size) = artifact.put(fs::File::open(scute.data_path())?)?;
    assert_eq!(digest.oci(), scute.descriptor.digest);
    assert_eq!(size, scute.descriptor.size);
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
        hex::encode(scute.root),
    );
    let manifest = Manifest {
        schema_version: 2,
        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
        artifact_type: "application/vnd.pichi.artifact.v1+json".into(),
        config: ConfigDescriptor::canonical(),
        layers: vec![Layer::Scute(scute.descriptor.clone())],
        annotations,
    };
    let fixture = Digest::sha256(b"synthetic publication fixture");
    let inventory = Inventory {
        version: 1,
        builder: inv::Builder {
            name: "conglobate".into(),
            version: "synthetic-validation-probe".into(),
            executable: Digest::from_oci(
                &conglobate::scute::digest_file(&PathBuf::from("/proc/self/exe"))?.0,
            )?,
        },
        // Explicit synthetic context: this is not an attested build claim.
        context: inv::Context {
            repository: "synthetic-fixture".into(),
            object_format: "sha1".into(),
            commit: vec![1; 20],
            subdirectory: ".".into(),
            tree: vec![2; 20],
        },
        inputs: vec![],
        execution: inv::Execution {
            architecture: "amd64".into(),
            recipes: BTreeMap::from([("refs.lock".into(), fixture.clone())]),
            transform: inv::Transform {
                source: "builtin".into(),
                digest: fixture,
                target_present: false,
                default_skipped: true,
            },
            network_steps: vec![],
        },
        outputs: inv::Outputs {
            manifest: Digest::sha256(&manifest.to_bytes()?),
            carapace_root: Some(scute.root.to_vec().into()),
            scutes: vec![inv::Scute {
                digest,
                origin: "produced".into(),
            }],
            pmi: None,
            dtb: None,
        },
    };
    artifact.finalize(&manifest, &inventory)?;
    artifact.publish(&args[3])?;
    let bytes = fs::read(args[3].join("inventory.cbor"))?;
    assert_eq!(Inventory::decode(&bytes)?.encode()?, inventory.encode()?);
    let stored = args[3]
        .join("image.oci/blobs/sha256")
        .join(&scute.descriptor.digest[7..]);
    assert_eq!(
        conglobate::scute::digest_file(&stored)?,
        (scute.descriptor.digest, scute.descriptor.size)
    );
    assert_eq!(conglobate::scute::digest_file(&args[0])?, before);
    println!("REAL_SCUTE_INVENTORY_PUBLICATION_PASS");
    Ok(())
}
