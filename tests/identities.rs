// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::process::{IdentityReceiver, IdentitySender, Process, Running};
use std::{
    io,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Fixture;
impl Fixture {
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(5)
    }
}

#[test]
fn trusted_child_identity_fixture() {
    if std::env::var_os("CONGLOBATE_TEST_IDENTITY_CHILD").is_none() {
        return;
    }
    IdentitySender::from_stdin()
        .unwrap()
        .send_current_until(Fixture::deadline())
        .unwrap();
    assert_eq!(
        std::io::Read::read(&mut io::stdin(), &mut [0u8; 1]).unwrap(),
        0
    );
}

#[test]
fn fresh_child_hands_off_its_identity_before_parent_reaping() {
    let (receiver, sender) = IdentityReceiver::pair().unwrap();
    let child: Process<Running> = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "trusted_child_identity_fixture", "--nocapture"])
        .env_clear()
        .env("CONGLOBATE_TEST_IDENTITY_CHILD", "1")
        .stdin(Stdio::from(sender))
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
        .into();
    let identity = receiver.receive_until(Fixture::deadline()).unwrap();
    assert!(
        child
            .wait_until(Fixture::deadline())
            .unwrap()
            .status()
            .success()
    );
    assert!(identity.has_exited().unwrap());
}

#[test]
fn an_unresponsive_peer_is_bounded_by_the_deadline() {
    let (receiver, _sender) = IdentityReceiver::pair().unwrap();
    let error = receiver
        .receive_until(Instant::now() + Duration::from_millis(20))
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn peer_loss_is_an_error_in_both_directions() {
    let (receiver, sender) = IdentityReceiver::pair().unwrap();
    drop(sender);
    assert_eq!(
        receiver
            .receive_until(Fixture::deadline())
            .unwrap_err()
            .kind(),
        io::ErrorKind::UnexpectedEof
    );
    let (receiver, sender) = IdentityReceiver::pair().unwrap();
    drop(receiver);
    assert!(sender.send_current_until(Fixture::deadline()).is_err());
}
