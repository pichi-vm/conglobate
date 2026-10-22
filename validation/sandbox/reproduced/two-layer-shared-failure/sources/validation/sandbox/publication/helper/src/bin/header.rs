//! Bounded authenticated invalid-header rejection through actual assembly.
#![forbid(unsafe_code)]
use conglobate::{
    cache::{Cache, ResolvedImage},
    scute::{Chain, Scute},
};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest};
use std::{collections::BTreeMap, fs, io::Read, path::Path, process::Command};
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new("/work");
    fs::write(root.join("raw"), [0x53; 4096])?;
    let imported = Scute::import(&root.join("raw"), &root.join("imported"))?;
    let canonical = fs::read(imported.data_path())?;
    for (name, offset, value) in [
        ("valid-control", 0, 0x7041_6e53_u32),
        ("magic", 0, 0),
        ("invalidated", 4, 0),
        ("version", 8, 2),
        ("chunk-size", 12, 16),
    ] {
        let directory = root.join(name);
        fs::create_dir(&directory)?;
        let mut bytes = canonical.clone();
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        fs::write(directory.join("data"), bytes)?;
        // Deliberately authenticate the altered bytes: digest mismatch must NOT
        // be the reason that malformed metadata is rejected.
        let sealed = Scute::seal(directory.join("data"), directory.join("hash"), [0; 32])?;
        let manifest = manifest(std::slice::from_ref(&sealed));
        let cache_dir = directory.join("cache");
        fs::create_dir_all(cache_dir.join("blobs/sha256"))?;
        fs::copy(
            sealed.data_path(),
            cache_dir
                .join("blobs/sha256")
                .join(&sealed.descriptor.digest[7..]),
        )?;
        let image = ResolvedImage {
            manifest_bytes: manifest.to_bytes()?,
            manifest,
            index: String::new(),
            manifest_digest: String::new(),
        };
        let chain = Chain::stage(&Cache::new(cache_dir), &image, &directory.join("staged"))?;
        assert_eq!(chain.root()?, sealed.root);
        println!("HEADER_BYTES_AUTHENTICATED {name}");
        match chain.assemble() {
            Ok(mut device) => {
                assert_eq!(name, "valid-control", "invalid header assembled");
                let mut actual = [0; 4096];
                device.read_exact(&mut actual)?;
                assert_eq!(actual, [0x53; 4096]);
                println!("HEADER_VALID_CONTROL_READ_PASS");
            }
            Err(error) => {
                assert_ne!(name, "valid-control");
                match &error {
                    conglobate::Error::Io(error) => {
                        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                        assert!(error.to_string().contains("snapshot header"), "{error}");
                    }
                    _ => panic!("wrong rejection: {error}"),
                }
                println!("AUTHENTICATED_HEADER_REJECTED {name}: {error}");
            }
        }
        assert!(
            Command::new("/bin/bash")
                .arg("/check-clean")
                .status()?
                .success()
        );
    }
    println!("AUTHENTICATED_HEADER_MATRIX_PASS");
    Ok(())
}
