//! Prepare owned inputs for the shared-block concurrency fixture.
#![forbid(unsafe_code)]
use conglobate::scute::Scute;
use std::{error::Error, fs, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let base = Scute::import(Path::new("/work/base"), Path::new("/shared/base"))?;
    fs::write("/shared/root", hex::encode(base.root))?;
    println!("SHARED_BLOCK_BASE_PREPARED");
    Ok(())
}
