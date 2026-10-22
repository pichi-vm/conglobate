//! Trusted ownership handoff for the bounded whole-build fixture.
#![forbid(unsafe_code)]
use nix::{mount::{mount, MsFlags}, poll::{poll, PollFd, PollFlags},
    sys::{prctl, signal::Signal}, unistd::{getpid, getppid}};
use std::{os::{fd::AsFd, unix::process::CommandExt}, process::{Command, Stdio}};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(getpid().as_raw(), 1);
    assert_eq!(getppid().as_raw(), 0);
    prctl::set_pdeathsig(Signal::SIGKILL)?;
    assert_eq!(prctl::get_pdeathsig()?, Some(Signal::SIGKILL));
    {
        let original_parent = std::io::stdin();
        let mut fds = [PollFd::new(original_parent.as_fd(), PollFlags::POLLIN)];
        if poll(&mut fds, 0_u16)? != 0 {
            return Err("original launcher is dead or handle is invalid".into());
        }
    }
    mount(Some("proc"), "/proc", Some("proc"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC, None::<&str>)?;
    println!("SAFE_RUST_OUTER_INIT_OWNERSHIP_ARMED");
    // Replace the control descriptor before passing into the fixture guardian.
    Err(Command::new("/usr/bin/python3").arg("/guardian.py")
        .stdin(Stdio::null()).exec().into())
}
