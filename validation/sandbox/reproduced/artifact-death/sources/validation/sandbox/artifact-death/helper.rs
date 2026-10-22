//! Real finalized artifact publication with copied-source stop gates.
#![forbid(unsafe_code)]
use conglobate::{artifact::Artifact, inventory::{self as inv, Digest, Inventory}};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest, PmiDescriptor};
use std::{collections::BTreeMap, fs, path::Path};
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


fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).expect("workspace");
    let root = Path::new(&root);
    let artifact = Artifact::new(&root.join("private"))?;
    artifact.put(&b"firmware"[..])?;
    let manifest = manifest();
    artifact.finalize(&manifest, &fixture(&manifest))?;
    let bytes = fs::read(root.join("private/inventory.cbor"))?;
    assert_eq!(Inventory::decode(&bytes)?.encode()?, bytes);
    artifact.publish(&root.join("output"))?;
    println!("ARTIFACT_PUBLICATION_RETURNED");
    Ok(())
}
