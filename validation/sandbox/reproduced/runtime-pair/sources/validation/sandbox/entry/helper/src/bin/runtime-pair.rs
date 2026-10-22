//! Two-root runtime-mount prototype. Roots are trusted paths; writers are stopped.
#![forbid(unsafe_code)]
use nix::{fcntl::{open, openat, OFlag}, mount::{mount, MsFlags}, sys::stat::Mode};
use std::os::fd::AsRawFd;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let roots: Vec<_> = std::env::args().skip(1).collect();
    if roots.len() != 2 { return Err("two trusted root paths required".into()); }
    let flags = OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    let mut targets = Vec::new();
    for (index, path) in roots.iter().enumerate() {
        let root = open(path.as_str(), flags, Mode::empty())?;
        for name in ["proc", "dev", "sys"] {
            let target = openat(&root, name, flags, Mode::empty())
                .map_err(|error| format!("root {index} runtime target {name}: {error}"))?;
            targets.push((name, target));
        }
    }
    println!("DUAL_RUNTIME_ALL_TARGETS_PINNED");
    for (name, target) in &targets {
        let path = format!("/proc/self/fd/{}", target.as_raw_fd());
        let (kind, options) = match *name {
            "proc" => ("proc", MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC),
            "dev" => ("tmpfs", MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC),
            "sys" => ("tmpfs", MsFlags::MS_RDONLY | MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC),
            _ => return Err("invalid fixed runtime target".into()),
        };
        mount(Some(kind), path.as_str(), Some(kind), options, None::<&str>)?;
    }
    println!("DUAL_RUNTIME_MOUNTS_INSTALLED");
    Ok(())
}
