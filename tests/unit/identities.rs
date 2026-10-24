// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::process::{Identified, LaunchFailure, Process, Supervision, WaitError};
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
    fn hold_until_terminated() {
        let (mut reader, _writer) = std::os::unix::net::UnixStream::pair().unwrap();
        // Keep the writer alive: only process termination ends this hold.
        std::io::Read::read_exact(&mut reader, &mut [0]).unwrap();
        panic!("hold unexpectedly released");
    }
    fn helper(hold: bool) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "tests::identities::trusted_child_identity_fixture",
                "--nocapture",
            ])
            .env_clear()
            .env("CONGLOBATE_TEST_IDENTITY_CHILD", "1")
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        if hold {
            command.env("CONGLOBATE_TEST_IDENTITY_HOLD", "1");
        }
        command
    }
}

#[test]
fn trusted_child_identity_fixture() {
    if std::env::var_os("CONGLOBATE_TEST_IDENTITY_CHILD").is_none() {
        return;
    }
    if std::env::var_os("CONGLOBATE_TEST_NO_IDENTITY").is_some() {
        Fixture::hold_until_terminated();
    }
    let _channel = Supervision::from_stdin()
        .unwrap()
        .exchange_until(Fixture::deadline())
        .unwrap();
    assert_eq!(
        std::io::Read::read(&mut io::stdin(), &mut [0u8; 1]).unwrap(),
        0
    );
    if std::env::var_os("CONGLOBATE_TEST_IDENTITY_HOLD").is_some() {
        Fixture::hold_until_terminated();
    }
}

#[test]
fn fresh_child_hands_off_its_identity_before_parent_reaping() {
    let (child, channel) =
        Process::<Identified>::launch_supervised(Fixture::helper(false), Fixture::deadline())
            .unwrap();
    assert!(
        child
            .wait_until(Fixture::deadline())
            .unwrap()
            .status()
            .success()
    );
    assert!(channel.peer_lost().unwrap());
}

#[test]
fn an_unresponsive_peer_is_bounded_by_the_deadline() {
    let (receiver, _sender) = Supervision::pair().unwrap();
    assert_eq!(
        receiver
            .exchange_until(Instant::now() + Duration::from_millis(20))
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
}

#[test]
fn peer_loss_rejects_production_identity_exchange() {
    let (receiver, sender) = Supervision::pair().unwrap();
    drop(sender);
    assert!(receiver.exchange_until(Fixture::deadline()).is_err());
    let (receiver, sender) = Supervision::pair().unwrap();
    drop(receiver);
    let mut command = Fixture::helper(false);
    command.stdin(Stdio::from(sender));
    assert!(!command.status().unwrap().success());
}

#[test]
fn identified_launch_preserves_identity_on_wait_timeout() {
    let (child, _channel) =
        Process::<Identified>::launch_supervised(Fixture::helper(true), Fixture::deadline())
            .unwrap();
    assert!(!child.has_exited().unwrap());
    let failure = child.wait_until(Instant::now()).unwrap_err();
    assert!(matches!(failure.error(), WaitError::Deadline));
    let (_, child) = failure.into_parts();
    assert!(!child.has_exited().unwrap());
    assert!(
        !child
            .terminate_until(Fixture::deadline())
            .unwrap()
            .status()
            .success()
    );
}

#[test]
fn identified_launch_collects_normal_completion() {
    let (child, _channel) =
        Process::<Identified>::launch_supervised(Fixture::helper(false), Fixture::deadline())
            .unwrap();
    assert!(
        child
            .wait_until(Fixture::deadline())
            .unwrap()
            .status()
            .success()
    );
}

#[test]
fn launch_failure_retains_started_children_but_not_spawn_failures() {
    assert!(matches!(
        Process::<Identified>::launch_supervised(Fixture::helper(true), Instant::now()),
        Err(LaunchFailure::Before(error)) if error.kind() == io::ErrorKind::TimedOut
    ));
    assert!(matches!(
        Process::<Identified>::launch_supervised(
            Command::new("/no-such-conglobate-helper"),
            Fixture::deadline()
        ),
        Err(LaunchFailure::Before(_))
    ));
    let failure =
        Process::<Identified>::launch_supervised(Command::new("/bin/true"), Fixture::deadline())
            .unwrap_err();
    let LaunchFailure::Started { source, process } = failure else {
        panic!("expected started child")
    };
    // The production exchange sends first: a peer exiting without reading may
    // reset the socket instead of producing EOF. Both retain the child owner.
    assert!(matches!(
        source.kind(),
        io::ErrorKind::UnexpectedEof | io::ErrorKind::ConnectionReset | io::ErrorKind::BrokenPipe
    ));
    assert!(
        process
            .wait_until(Fixture::deadline())
            .unwrap()
            .status()
            .success()
    );

    let mut command = Fixture::helper(false);
    command.env("CONGLOBATE_TEST_NO_IDENTITY", "1");
    let failure = Process::<Identified>::launch_supervised(
        command,
        Instant::now() + Duration::from_millis(30),
    )
    .unwrap_err();
    let LaunchFailure::Started { source, process } = failure else {
        panic!("expected started child")
    };
    assert_eq!(source.kind(), io::ErrorKind::TimedOut);
    assert!(
        !process
            .terminate_until(Fixture::deadline())
            .unwrap()
            .status()
            .success()
    );
}

#[test]
fn dropping_identified_helper_requests_termination_and_reaps() {
    let (child, _channel) =
        Process::<Identified>::launch_supervised(Fixture::helper(true), Fixture::deadline())
            .unwrap();
    // Independent test-only witness, acquired while this wait-owned child is
    // alive. Production identity acquisition uses only the helper handoff.
    let witness = rustix::process::pidfd_open(
        rustix::process::Pid::from_raw(child.id() as i32).unwrap(),
        rustix::process::PidfdFlags::empty(),
    )
    .unwrap();
    drop(child);
    let mut fds = [rustix::event::PollFd::new(
        &witness,
        rustix::event::PollFlags::IN,
    )];
    rustix::event::poll(
        &mut fds,
        Some(&rustix::event::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )
    .unwrap();
    // HUP additionally establishes reaping, rather than only exit readiness.
    assert!(fds[0].revents().contains(rustix::event::PollFlags::HUP));
}
