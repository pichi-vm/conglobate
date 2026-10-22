//! Bounded fixture commit, called only after trusted owner/resource checks.
#![forbid(unsafe_code)]
use nix::fcntl::{renameat2, RenameFlags};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parent = std::fs::File::open("/work")?;
    renameat2(&parent, "published", &parent, "result", RenameFlags::RENAME_NOREPLACE)?;
    parent.sync_all()?;
    Ok(())
}
