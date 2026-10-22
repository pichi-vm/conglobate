// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::process::{Process, Running, Supervision, Unpaired};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Fixture;
impl Fixture {
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }
    fn hold_until_terminated() {
        let (mut reader, _writer) = std::os::unix::net::UnixStream::pair().unwrap();
        // Keep the writer alive: only process termination ends this hold.
        std::io::Read::read_exact(&mut reader, &mut [0]).unwrap();
        panic!("hold unexpectedly released");
    }
    fn command(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "trusted_supervision_fixture", "--nocapture"])
            .env_clear()
            .env("CONGLOBATE_SUPERVISION_FIXTURE", mode)
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        command
    }
    fn start(mode: &str) -> (Supervision<Unpaired>, Process<Running>) {
        let (parent, child) = Supervision::pair().unwrap();
        let mut command = Self::command(mode);
        command.stdin(Stdio::from(child));
        let process = command.spawn().unwrap().into();
        drop(command);
        (parent, process)
    }
}
#[test]
fn trusted_supervision_fixture() {
    let Some(mode) = std::env::var_os("CONGLOBATE_SUPERVISION_FIXTURE") else {
        return;
    };
    if mode == "unresponsive" {
        Fixture::hold_until_terminated();
    }
    let watch = Supervision::from_stdin()
        .unwrap()
        .exchange_until(Fixture::deadline())
        .unwrap();
    if mode == "exit" {
        std::process::exit(0);
    }
    if mode == "hold" {
        Fixture::hold_until_terminated();
        return;
    }
    watch.wait_for_loss_until(Fixture::deadline()).unwrap();
}
#[test]
fn paired_peer_exit_is_observable_without_destructors() {
    let (parent, child) = Fixture::start("exit");
    let watch = parent.exchange_until(Fixture::deadline()).unwrap();
    assert!(
        child
            .wait_until(Fixture::deadline())
            .unwrap()
            .status()
            .success()
    );
    assert!(watch.peer_lost().unwrap());
}
#[test]
fn live_peer_then_forced_exit_is_observable() {
    let (parent, child) = Fixture::start("hold");
    let watch = parent.exchange_until(Fixture::deadline()).unwrap();
    assert!(!watch.peer_lost().unwrap());
    assert_eq!(
        watch
            .wait_for_loss_until(Instant::now() + Duration::from_millis(20))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::TimedOut
    );
    assert!(!watch.peer_lost().unwrap());
    child.terminate_until(Fixture::deadline()).unwrap();
    assert!(watch.peer_lost().unwrap());
}
#[test]
fn normal_drop_cancels_a_live_peer() {
    let (parent, child) = Fixture::start("cancel");
    let watch = parent.exchange_until(Fixture::deadline()).unwrap();
    drop(watch);
    assert!(
        child
            .wait_until(Fixture::deadline())
            .unwrap()
            .status()
            .success()
    );
}
#[test]
fn unresponsive_exchange_obeys_one_deadline() {
    let (parent, _child) = Supervision::pair().unwrap();
    let error = parent
        .exchange_until(Instant::now() + Duration::from_millis(20))
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
}

#[test]
fn supervised_launch_retains_identity_and_wait_ownership() {
    let (child, watch) =
        Process::launch_supervised(Fixture::command("hold"), Fixture::deadline()).unwrap();
    assert!(!child.has_exited().unwrap());
    assert!(!watch.peer_lost().unwrap());
    child.terminate_until(Fixture::deadline()).unwrap();
    watch.wait_for_loss_until(Fixture::deadline()).unwrap();
}
#[test]
fn supervised_exchange_failure_retains_the_started_child() {
    let command = Fixture::command("unresponsive");
    let failure = Process::launch_supervised(command, Instant::now() + Duration::from_millis(20))
        .unwrap_err();
    let conglobate::process::LaunchFailure::Started { source, process } = failure else {
        panic!("expected a started helper");
    };
    assert_eq!(source.kind(), std::io::ErrorKind::TimedOut);
    process.terminate_until(Fixture::deadline()).unwrap();
}
