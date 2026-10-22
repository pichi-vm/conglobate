//! Exercise production scratch-lease recovery inside a disposable guest.
#![forbid(unsafe_code)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let scratch = conglobate::mounts::Scratch::new()?;
    assert!(scratch.path().is_dir());
    drop(scratch);
    println!("SCRATCH_RECOVERY_RETURNED");
    Ok(())
}
