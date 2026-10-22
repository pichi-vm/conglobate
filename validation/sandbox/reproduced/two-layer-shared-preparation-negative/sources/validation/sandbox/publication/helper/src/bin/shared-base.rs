//! Prepare owned two-layer inputs for the shared-block concurrency fixture.
#![forbid(unsafe_code)]
use conglobate::{
    mounts::Scratch,
    scute::{Chain, Scute},
    snapshot::Snapshot,
};
use std::{error::Error, fs, path::Path, process::Command};

fn main() -> Result<(), Box<dyn Error>> {
    let base = Scute::import(Path::new("/work/base"), Path::new("/shared/base"))?;
    let parent = base.root;
    let chain = Chain { scutes: vec![base] };
    let scratch = Scratch::new()?;
    let snapshot = Snapshot::create(chain.assemble()?, parent, &scratch.path().join("layer"))?;
    assert!(
        Command::new("mount")
            .arg(snapshot.device_path())
            .arg("/work/view")
            .status()?
            .success()
    );
    fs::write("/work/view/shared-layer", b"inherited second layer\n")?;
    assert!(Command::new("umount").arg("/work/view").status()?.success());
    let layer = snapshot.finish()?;
    assert_eq!(layer.parent, parent);
    assert_ne!(layer.root, parent);
    fs::create_dir("/shared/layer")?;
    fs::copy(layer.data_path(), "/shared/layer/data")?;
    fs::copy(scratch.path().join("layer/hash"), "/shared/layer/hash")?;
    fs::write("/shared/root", hex::encode(layer.root))?;
    println!("SHARED_BLOCK_TWO_LAYER_BASE_PREPARED");
    println!("SHARED_BLOCK_BASE_PREPARED");
    Ok(())
}
