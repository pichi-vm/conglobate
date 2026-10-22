//! Bounded userspace-only characterization of authenticated exception metadata.
#![forbid(unsafe_code)]
use conglobate::{
    cache::{Cache, ResolvedImage},
    scute::{Chain, Scute},
};
use pichi_artifact::{ConfigDescriptor, Layer, Manifest};
use std::{collections::BTreeMap, fs, path::Path};
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
    fs::write(root.join("raw"), [0x53; 8192])?;
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
    ] {
        let directory = root.join(name);
        fs::create_dir(&directory)?;
        let mut bytes = canonical.clone();
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
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
        // Deliberately never call assemble or expose these test bytes to DM.
        println!("EXCEPTION_METADATA_AUTHENTICATED_WITHOUT_SEMANTIC_CHECK {name}");
    }
    println!("EXCEPTION_METADATA_USERSPACE_CHARACTERIZATION_COMPLETE");
    Ok(())
}
