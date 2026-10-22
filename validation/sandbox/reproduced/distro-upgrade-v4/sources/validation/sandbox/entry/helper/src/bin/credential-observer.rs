//! Owned set-ID test executable: report only this process's credentials.
#![forbid(unsafe_code)]
use nix::unistd::{getuid, geteuid, getgid, getegid};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let status = std::fs::read_to_string("/proc/self/status")?;
    let field = |name: &str| -> Result<&str, Box<dyn std::error::Error>> {
        status.lines().find_map(|line| line.strip_prefix(name)).map(str::trim)
            .ok_or_else(|| "missing credential field".into())
    };
    println!("{} {} {} {} {} {} {}", getuid(), geteuid(), getgid(), getegid(),
        field("CapEff:")?, field("NoNewPrivs:")?, field("Seccomp:")?);
    Ok(())
}
