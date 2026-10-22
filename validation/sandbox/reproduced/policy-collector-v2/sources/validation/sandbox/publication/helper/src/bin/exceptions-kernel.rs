//! Bounded guest-only snapshot exception read and cleanup characterization.
#![forbid(unsafe_code)]
use conglobate::{
    cache::{Cache, ResolvedImage},
    scute::{Chain, Scute},
};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest};
use std::{collections::BTreeMap, fs, os::unix::fs::FileExt, path::Path, process::Command};
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
    let argument = std::env::args().nth(1).expect("new fixture directory");
    let root = Path::new(&argument);
    fs::create_dir(root)?;
    let mut raw = vec![0x53; 4096];
    raw.extend_from_slice(&[0x71; 4096]);
    fs::write(root.join("raw"), &raw)?;
    let imported = Scute::import(&root.join("raw"), &root.join("imported"))?;
    let canonical = fs::read(imported.data_path())?;
    assert_eq!(canonical.len(), 4 * 4096);
    let get = |offset: usize| u64::from_le_bytes(canonical[offset..offset + 8].try_into().unwrap());
    assert_eq!((get(4096), get(4104), get(4112), get(4120)), (0, 2, 1, 3));
    assert_eq!(get(4136), 0);
    for (name, offset, value) in [
        ("valid-control", 4104, 2),
        ("terminator-old-ignored", 4128, u64::MAX),
        ("metadata-as-data", 4104, 1),
        ("data-past-store", 4104, 4),
        ("origin-past-64g", 4096, 64 * 1024 * 1024 * 1024 / 4096),
        ("duplicate-origin", 4112, 0),
        ("duplicate-data", 4120, 2),
        ("full-area-eof", 0, 0),
        ("full-area-next-terminator", 0, 0),
    ] {
        let directory = root.join(name);
        fs::create_dir(&directory)?;
        let mut bytes = canonical.clone();
        if name.starts_with("full-area-") {
            // Fill exactly one owned 4096-byte metadata area (256 records).
            // All exceptions point to the same known data chunk, avoiding any
            // arbitrary device reads. The next metadata area is chunk 258.
            for index in 0..256_usize {
                let at = 4096 + index * 16;
                bytes[at..at + 8].copy_from_slice(&(index as u64).to_le_bytes());
                bytes[at + 8..at + 16].copy_from_slice(&2_u64.to_le_bytes());
            }
            if name == "full-area-next-terminator" {
                // Positive boundary control: the next area exists and begins
                // with the zero new_chunk terminator. Maximum fixture ~1MiB.
                bytes.resize(259 * 4096, 0);
            }
        } else {
            bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }
        fs::write(directory.join("data"), &bytes)?;
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
        assert_eq!(fs::read(chain.scutes[0].data_path())?, bytes);
        println!("KERNEL_EXCEPTION_BYTES_AUTHENTICATED {name}");
        match chain.assemble() {
            Err(error) => {
                assert!(!matches!(
                    name,
                    "valid-control" | "terminator-old-ignored" | "full-area-next-terminator"
                ));
                println!("KERNEL_EXCEPTION_ASSEMBLY_REJECTED {name}: {error}");
            }
            Ok(device) => {
                assert_ne!(
                    name, "full-area-eof",
                    "missing next metadata area was accepted"
                );
                println!("KERNEL_EXCEPTION_ASSEMBLED {name}");
                for index in 0..2_u64 {
                    let mut actual = [0; 4096];
                    match device.read_exact_at(&mut actual, index * 4096) {
                        Ok(()) => {
                            let classification = if actual == [0x53; 4096] {
                                "data-A"
                            } else if actual == [0x71; 4096] {
                                "data-B"
                            } else if actual == [0; 4096] {
                                "zero-origin"
                            } else if actual == bytes[4096..8192] {
                                "exception-metadata"
                            } else {
                                panic!("read escaped known fixture byte classes: {name} {index}")
                            };
                            if matches!(name, "valid-control" | "terminator-old-ignored") {
                                assert_eq!(
                                    &actual,
                                    &raw[index as usize * 4096..(index as usize + 1) * 4096]
                                );
                            }
                            if name == "full-area-next-terminator" {
                                assert_eq!(actual, [0x53; 4096]);
                            }
                            println!("KERNEL_EXCEPTION_READ {name} {index} {classification}");
                        }
                        Err(error) => {
                            assert!(!matches!(
                                name,
                                "valid-control"
                                    | "terminator-old-ignored"
                                    | "full-area-next-terminator"
                            ));
                            println!("KERNEL_EXCEPTION_READ_ERROR {name} {index}: {error}");
                        }
                    }
                }
            }
        }
        assert_eq!(fs::read(sealed.data_path())?, bytes);
        assert!(
            Command::new("/bin/bash")
                .arg("/check-clean")
                .status()?
                .success()
        );
        println!("KERNEL_EXCEPTION_CASE_COMPLETE {name}");
    }
    println!("KERNEL_EXCEPTION_MATRIX_COMPLETE");
    Ok(())
}
