//! Bounded fixture commit, called only after trusted owner/resource checks.
#![forbid(unsafe_code)]
use nix::fcntl::{renameat2, RenameFlags};
use std::path::Path;
fn gate(parent: &Path, stage: &str) -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("COMMIT_STOP").as_deref() == Ok(stage) {
        std::fs::write(parent.join("reached"), stage)?;
        nix::sys::signal::raise(nix::sys::signal::Signal::SIGSTOP)?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).unwrap_or_else(|| "/work".into());
    let path = Path::new(&path);
    let parent = std::fs::File::open(path)?;
    gate(path, "before-rename")?;
    renameat2(&parent, "published", &parent, "result", RenameFlags::RENAME_NOREPLACE)?;
    gate(path, "after-rename")?;
    parent.sync_all()?;
    gate(path, "after-sync")?;
    println!("OUTER_COMMIT_RETURNED");
    Ok(())
}
