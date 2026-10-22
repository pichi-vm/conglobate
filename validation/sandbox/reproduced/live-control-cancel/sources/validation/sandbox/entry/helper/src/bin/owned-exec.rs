//! Diagnostic trusted-child link. No credential changes or recipe execution.
#![forbid(unsafe_code)]
use nix::{poll::{poll, PollFd, PollFlags}, sys::{prctl, signal::Signal}};
use std::{os::{fd::AsFd, unix::process::CommandExt}, process::{Command, Stdio}};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let executable = args.next().ok_or("trusted executable required")?;
    prctl::set_pdeathsig(Signal::SIGKILL)?;
    assert_eq!(prctl::get_pdeathsig()?, Some(Signal::SIGKILL));
    {
        // A trusted, single-threaded parent supplies its own pinned pidfd.
        let original_parent = std::io::stdin();
        let mut fds = [PollFd::new(original_parent.as_fd(), PollFlags::POLLIN)];
        if poll(&mut fds, 0_u16)? != 0 {
            return Err("original parent is dead or handle is invalid".into());
        }
    }
    Err(Command::new(executable).args(args).stdin(Stdio::null())
        .env_clear().envs([("PATH", "/usr/bin:/usr/sbin"), ("HOME", "/"), ("LC_ALL", "C")])
        .exec().into())
}
