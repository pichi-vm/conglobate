//! Safe-Rust parent ownership handoff probe; never executes recipe code.
#![forbid(unsafe_code)]
use nix::{poll::{poll, PollFd, PollFlags}, sys::{prctl, signal::{raise, Signal}}};
use std::{fs, os::fd::AsFd, path::Path};
fn stop(stage: &str, selected: &str) -> Result<(), Box<dyn std::error::Error>> {
    if stage == selected {
        fs::write("/work/stopped", stage)?;
        raise(Signal::SIGSTOP)?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stage = std::env::args().nth(1).expect("stage");
    stop("before-arm", &stage)?;
    prctl::set_pdeathsig(Signal::SIGKILL)?;
    assert_eq!(prctl::get_pdeathsig()?, Some(Signal::SIGKILL));
    stop("after-arm", &stage)?;
    // Trusted launcher passes its already-open pidfd as fd 0. stdin().as_fd()
    // borrows this descriptor safely, without FromRawFd or other unsafe code.
    let parent = std::io::stdin();
    let mut descriptors = [PollFd::new(parent.as_fd(), PollFlags::POLLIN)];
    if poll(&mut descriptors, 0_u16)? != 0 {
        return Err("original parent is dead or its handle is invalid".into());
    }
    fs::write("/work/admitted", "original parent alive at handoff\n")?;
    stop("after-check", &stage)?;
    assert!(!Path::new("/work/finished").exists());
    fs::write("/work/finished", "trusted probe finished; no recipe executed\n")?;
    Ok(())
}
