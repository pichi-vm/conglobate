//! Import an owned filesystem fixture without mounts or host device operations.
#![forbid(unsafe_code)]
use std::{error::Error, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len() != 2 { return Err("raw input and new output directory required".into()); }
    let scute = conglobate::scute::Scute::import(&args[0], &args[1])?;
    println!("{}", hex::encode(scute.root));
    Ok(())
}
