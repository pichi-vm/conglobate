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
    let child = Fixture::child("/bin/sleep", &["30"]);
    let pid = child.id();
    let failure = child.wait_until(Instant::now()).unwrap_err();
    assert!(matches!(failure.error(), WaitError::Deadline));
    let (_, child) = failure.into_parts();
    assert_eq!(child.id(), pid);
    let reaped = child.terminate_until(Fixture::deadline()).unwrap();
    assert!(!reaped.status().success());
}
