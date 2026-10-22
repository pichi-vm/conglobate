//! Safe-Rust parent ownership handoff probe; never executes recipe code.
#![forbid(unsafe_code)]
use nix::{poll::{poll, PollFd, PollFlags}, sys::{prctl, signal::{raise, Signal}}};
use std::{fs, os::fd::AsFd, path::Path};
fn stop(stage: &str, selected: &str) -> Result<(), Box<dyn std::error::Error>> {
    if stage == selected {
        fs::write("/work/stopped", stage)?;
        if nix::unistd::getpid().as_raw() == 1 {
            // Namespace init ignores self-sent stop; the ancestor observer stops it.
            while !Path::new("/work/resume").exists() {
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        } else {
            raise(Signal::SIGSTOP)?;
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage = std::env::args().nth(1).expect("stage");
    fs::write("/work/identity", format!("{} {}", nix::unistd::getpid(), nix::unistd::getppid()))?;
    stop("before-arm", &stage)?;
    let exec_child = std::env::args().nth(3).as_deref() == Some("exec-child");
    if !exec_child {
        prctl::set_pdeathsig(Signal::SIGKILL)?;
    }
    assert_eq!(prctl::get_pdeathsig()?, Some(Signal::SIGKILL));
    if stage == "exec-after-check" && !exec_child {
        use std::os::unix::process::CommandExt;
        return Err(std::process::Command::new(std::env::current_exe()?)
            .args([&stage, &std::env::args().nth(2).unwrap(), "exec-child"])
            .exec().into());
    }
    if stage.starts_with("credential-") {
        use nix::unistd::{setresgid, setresuid, Gid, Uid};
        let gid = Gid::from_raw(100000);
        let uid = Uid::from_raw(100000);
        setresgid(gid, gid, gid)?;
        assert_eq!(prctl::get_pdeathsig()?, None);
        // Independently establish that the subsequent UID change clears it.
        prctl::set_pdeathsig(Signal::SIGKILL)?;
        setresuid(uid, uid, uid)?;
        assert_eq!(prctl::get_pdeathsig()?, None);
        if stage == "credential-cleared" {
            stop(&stage, &stage)?;
            fs::write("/work/finished", "unlinked diagnostic survived parent death")?;
            return Ok(());
        }
        stop("credential-before-arm", &stage)?;
        prctl::set_pdeathsig(Signal::SIGKILL)?;
        assert_eq!(prctl::get_pdeathsig()?, Some(Signal::SIGKILL));
    }
    stop("after-arm", &stage)?;
    // Trusted launcher passes its already-open pidfd as fd 0. stdin().as_fd()
    // borrows this descriptor safely, without FromRawFd or other unsafe code.
    let parent = std::io::stdin();
    let mut descriptors = [PollFd::new(parent.as_fd(), PollFlags::POLLIN)];
    if poll(&mut descriptors, 0_u16)? != 0 {
        return Err("original parent is dead or its handle is invalid".into());
    }
    fs::write("/work/admitted", "original parent alive at handoff\n")?;
    if matches!(stage.as_str(), "exec-after-check" | "credential-after-check") {
        stop(&stage, &stage)?;
    } else {
        stop("after-check", &stage)?;
    }
    assert!(!Path::new("/work/finished").exists());
    fs::write("/work/finished", "trusted probe finished; no recipe executed\n")?;
    Ok(())
}
