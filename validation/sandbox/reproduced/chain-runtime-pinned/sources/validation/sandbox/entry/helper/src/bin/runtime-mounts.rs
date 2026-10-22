//! Safe-Rust runtime-directory pinning prototype; requires stopped image writers.
#![forbid(unsafe_code)]
use nix::{fcntl::{open, openat, OFlag}, mount::{mount, MsFlags}, sys::stat::Mode};
use std::os::fd::AsRawFd;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("root missing")?;
    let flags = OFlag::O_PATH | OFlag::O_DIRECTORY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC;
    let root = open(path.as_str(), flags, Mode::empty())?;
    // These are fixed single-component names. No attacker-chosen intermediate
    // component is resolved, and all targets are pinned before any runtime mount.
    let mut targets = Vec::new();
    for name in ["proc", "dev", ".old"] {
        let fd = openat(&root, name, flags, Mode::empty())
            .map_err(|error| format!("runtime target {name}: {error}"))?;
        targets.push(fd);
    }
    println!("RUNTIME_TARGETS_PINNED");
    let proc_target = format!("/proc/self/fd/{}", targets[0].as_raw_fd());
    let dev_target = format!("/proc/self/fd/{}", targets[1].as_raw_fd());
    mount(Some("proc"), proc_target.as_str(), Some("proc"),
          MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC, None::<&str>)?;
    mount(Some("tmpfs"), dev_target.as_str(), Some("tmpfs"),
          MsFlags::MS_NOSUID | MsFlags::MS_NOEXEC, None::<&str>)?;
    println!("PINNED_RUNTIME_MOUNTS_INSTALLED");
    Ok(())
}
