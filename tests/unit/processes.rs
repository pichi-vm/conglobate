// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::process::{Process, Running, WaitError};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Fixture;
impl Fixture {
    fn child(program: &str, args: &[&str]) -> Process<Running> {
        Command::new(program)
            .args(args)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap()
            .into()
    }
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }
    fn blocked_child() -> (Process<Running>, std::process::ChildStdin) {
        let mut child = Command::new("/bin/sh")
            .args(["-c", "read line; exit 37"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        (child.into(), input)
    }
}
#[test]
fn reaped_state_preserves_success_and_failure_status() {
    for code in [0, 37] {
        let command = format!("exit {code}");
        let child = Fixture::child("/bin/sh", &["-c", &command]);
        let reaped = child.wait_until(Fixture::deadline()).unwrap();
        assert_eq!(reaped.status().code(), Some(code));
    }
}
#[test]
fn expired_wait_retains_child_for_termination_and_reaping() {
    let (child, _input) = Fixture::blocked_child();
    let pid = child.id();
    let failure = child.wait_until(Instant::now()).unwrap_err();
    assert!(matches!(failure.error(), WaitError::Deadline));
    let (_, child) = failure.into_parts();
    assert_eq!(child.id(), pid);
    let reaped = child.terminate_until(Fixture::deadline()).unwrap();
    assert!(!reaped.status().success());
}

#[test]
fn expired_wait_can_resume_after_explicit_release() {
    let (child, input) = Fixture::blocked_child();
    let failure = child.wait_until(Instant::now()).unwrap_err();
    assert!(matches!(failure.error(), WaitError::Deadline));
    let (_, child) = failure.into_parts();
    // EOF releases read regardless of scheduling; no fixed delay is readiness.
    drop(input);
    let reaped = child.wait_until(Fixture::deadline()).unwrap();
    assert_eq!(reaped.status().code(), Some(37));
}
