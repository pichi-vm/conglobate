// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ProcessObservation, identity::IdentitySocket};
use rustix::{
    event::{PollFd, PollFlags, poll},
    fd::OwnedFd,
    net::{AddressFamily, RecvFlags, SocketFlags, SocketType, recv, socketpair},
};
use std::{io, process::Stdio, time::Instant};

mod control;
mod joining;
mod volume;
pub(crate) use joining::Joined;

mod private {
    pub(crate) trait Sealed {}
}
pub(crate) trait SupervisionState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub(crate) struct Unpaired;
#[derive(Debug)]
pub(crate) struct Paired;
impl private::Sealed for Unpaired {}
impl private::Sealed for Paired {}
impl SupervisionState for Unpaired {
    type Resources = ();
}
impl SupervisionState for Paired {
    type Resources = ProcessObservation;
}

#[derive(Debug)]
struct ControlSocket(IdentitySocket);
impl Drop for ControlSocket {
    fn drop(&mut self) {
        // Normal cancellation must not wait for transient inherited FD copies.
        // Peer pidfds separately detect process death without destructor execution.
        let _ = rustix::net::shutdown(&self.0.0, rustix::net::Shutdown::Both);
    }
}
/// Mutual stable-identity observation between trusted supervisor/helper code.
/// Pairing is not readiness, domain joining, containment or command admission.
/// Neither side may expose this endpoint or its process identity to commands.
/// Both endpoints progress Unpaired -> Paired through identity exchange, then
/// Joined through domain joining. Each transition consumes the prior owner;
/// paired peer observation alone never permits image-code execution.
///
#[derive(Debug)]
pub(crate) struct Supervision<S: SupervisionState> {
    socket: ControlSocket,
    peer: S::Resources,
}
/// Startup endpoint transferred exclusively to a trusted helper's stdin.
#[derive(Debug)]
pub(crate) struct SupervisionChild(OwnedFd);
impl From<SupervisionChild> for Stdio {
    fn from(child: SupervisionChild) -> Self {
        child.0.into()
    }
}
impl Supervision<Unpaired> {
    pub(crate) fn pair() -> io::Result<(Self, SupervisionChild)> {
        let (parent, child) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )?;
        Ok((
            Self {
                socket: ControlSocket(IdentitySocket(parent)),
                peer: (),
            },
            SupervisionChild(child),
        ))
    }
    /// Dedicated trusted helper startup only, before concurrent stdin access.
    /// Moves stdin to a CLOEXEC descriptor and replaces stdin with /dev/null.
    pub(crate) fn from_stdin() -> io::Result<Self> {
        Ok(Self {
            socket: ControlSocket(IdentitySocket::from_stdin()?),
            peer: (),
        })
    }
    /// Both peers send their own pidfd, then receive the other under one deadline.
    /// Error closes/cancels the channel; separately owned children still need reaping.
    pub(crate) fn exchange_until(self, deadline: Instant) -> io::Result<Supervision<Paired>> {
        self.exchange_with(&deadline)
    }
    pub(super) fn exchange_with(
        self,
        wait: &impl super::identity::ExchangeWait,
    ) -> io::Result<Supervision<Paired>> {
        self.socket.0.send_current_with(wait)?;
        let peer = self.socket.0.receive_with(wait)?;
        Ok(Supervision {
            socket: self.socket,
            peer,
        })
    }
}
pub(crate) trait ObservedState: SupervisionState<Resources = ProcessObservation> {}
impl ObservedState for Paired {}
impl ObservedState for Joined {}
impl<S: ObservedState> Supervision<S> {
    pub(super) fn duplicate_observation(&self) -> io::Result<ProcessObservation> {
        Ok(ProcessObservation {
            identity: self.peer.identity.try_clone()?,
        })
    }

    /// Wait on both the stable process identity and the private channel. Timeout
    /// is not evidence of liveness; no resource cleanup or reaping happens here.
    pub(crate) fn wait_for_loss_until(&self, deadline: Instant) -> io::Result<()> {
        loop {
            if self.peer_lost()? {
                return Ok(());
            }
            let timeout = crate::deadline::Deadline::at(deadline)
                .remaining("supervision wait deadline expired")?;
            let mut descriptors = [
                PollFd::new(&self.socket.0.0, PollFlags::IN),
                PollFd::new(&self.peer.identity, PollFlags::IN),
            ];
            match poll(&mut descriptors, Some(&timeout)) {
                Ok(_) | Err(rustix::io::Errno::INTR) => {}
                Err(error) => return Err(error.into()),
            }
        }
    }

    /// Nonblocking observation. True means cancel and begin ordered teardown,
    /// never that descendants are quiescent. Errors must also fail the build.
    /// Outside an explicit domain-join exchange, no messages are accepted. Invalid
    /// traffic remains queued so subsequent observations cannot report healthy.
    pub(crate) fn peer_lost(&self) -> io::Result<bool> {
        if self.peer.has_exited()? {
            return Ok(true);
        }
        let mut byte = [0u8];
        match recv(
            &self.socket.0.0,
            &mut byte,
            RecvFlags::DONTWAIT | RecvFlags::PEEK,
        ) {
            Ok((0, _)) => Ok(true),
            Ok(_) => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected supervision message",
            )),
            Err(rustix::io::Errno::AGAIN) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process::{Process, Running};
    use rustix::{
        net::{SendFlags, send},
        process::{Pid, PidfdFlags, pidfd_open},
    };
    use std::{process::Command, time::Duration};

    #[derive(Debug)]
    struct Fixture;
    impl Fixture {
        fn deadline() -> Instant {
            Instant::now() + Duration::from_secs(5)
        }
        fn paired() -> (Supervision<Paired>, Supervision<Paired>) {
            let (parent, child) = Supervision::pair().unwrap();
            let helper = std::thread::spawn(move || {
                let peer = Supervision::<Unpaired> {
                    socket: ControlSocket(IdentitySocket(child.0)),
                    peer: (),
                };
                peer.exchange_until(Self::deadline()).unwrap()
            });
            let parent = parent.exchange_until(Self::deadline()).unwrap();
            (parent, helper.join().unwrap())
        }
    }
    #[test]
    fn invalid_traffic_cannot_be_consumed_into_a_healthy_state() {
        let (parent, peer) = Fixture::paired();
        send(&peer.socket.0.0, b"unexpected", SendFlags::NOSIGNAL).unwrap();
        for _ in 0..2 {
            assert_eq!(
                parent.peer_lost().unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        assert_eq!(
            parent
                .wait_for_loss_until(Fixture::deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn cancellation_does_not_depend_on_the_last_endpoint_copy_closing() {
        let (parent, peer) = Fixture::paired();
        let _duplicate = peer.socket.0.0.try_clone().unwrap();
        drop(peer);
        assert!(parent.peer_lost().unwrap());
    }
    #[test]
    fn process_exit_is_detected_even_when_the_socket_stays_open() {
        // Deliberately keep the peer endpoint in this process to isolate pidfd
        // observation from EOF. This is a mechanism test, not a startup receipt.
        let (socket, _peer_endpoint) = Supervision::pair().unwrap();
        let mut child = Command::new("/bin/cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        // Hold stdin open without data until explicit process termination.
        let _input = child.stdin.take().unwrap();
        let pid = Pid::from_raw(child.id() as i32).unwrap();
        let child: super::super::Process<super::super::Running> = child.into();
        // Exclusive wait ownership keeps this child's PID from reuse while
        // obtaining the test identity; production uses the child's self-pidfd.
        let identity = pidfd_open(pid, PidfdFlags::empty()).unwrap();
        let watch = Supervision::<Paired> {
            socket: socket.socket,
            peer: ProcessObservation { identity },
        };
        assert!(!watch.peer_lost().unwrap());
        child.terminate_until(Fixture::deadline()).unwrap();
        watch.wait_for_loss_until(Fixture::deadline()).unwrap();
        assert!(watch.peer_lost().unwrap());
    }
    #[derive(Debug)]
    struct ChildFixture;
    impl ChildFixture {
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
                .args([
                    "--exact",
                    "process::supervision::tests::trusted_supervision_fixture",
                    "--nocapture",
                ])
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
            ChildFixture::hold_until_terminated();
        }
        let watch = Supervision::from_stdin()
            .unwrap()
            .exchange_until(ChildFixture::deadline())
            .unwrap();
        if mode == "exit" {
            std::process::exit(0);
        }
        if mode == "hold" {
            ChildFixture::hold_until_terminated();
            return;
        }
        watch.wait_for_loss_until(ChildFixture::deadline()).unwrap();
    }
    #[test]
    fn paired_peer_exit_is_observable_without_destructors() {
        let (parent, child) = ChildFixture::start("exit");
        let watch = parent.exchange_until(ChildFixture::deadline()).unwrap();
        assert!(
            child
                .wait_until(ChildFixture::deadline())
                .unwrap()
                .status()
                .success()
        );
        assert!(watch.peer_lost().unwrap());
    }
    #[test]
    fn live_peer_then_forced_exit_is_observable() {
        let (parent, child) = ChildFixture::start("hold");
        let watch = parent.exchange_until(ChildFixture::deadline()).unwrap();
        assert!(!watch.peer_lost().unwrap());
        assert_eq!(
            watch
                .wait_for_loss_until(Instant::now() + Duration::from_millis(20))
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::TimedOut
        );
        assert!(!watch.peer_lost().unwrap());
        child.terminate_until(ChildFixture::deadline()).unwrap();
        assert!(watch.peer_lost().unwrap());
    }
    #[test]
    fn normal_drop_cancels_a_live_peer() {
        let (parent, child) = ChildFixture::start("cancel");
        let watch = parent.exchange_until(ChildFixture::deadline()).unwrap();
        drop(watch);
        assert!(
            child
                .wait_until(ChildFixture::deadline())
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
            Process::launch_supervised(ChildFixture::command("hold"), ChildFixture::deadline())
                .unwrap();
        assert!(!child.has_exited().unwrap());
        assert!(!watch.peer_lost().unwrap());
        child.terminate_until(ChildFixture::deadline()).unwrap();
        watch.wait_for_loss_until(ChildFixture::deadline()).unwrap();
    }
    #[test]
    fn supervised_exchange_failure_retains_the_started_child() {
        let command = ChildFixture::command("unresponsive");
        // This budget includes spawning the helper, unlike the already-paired
        // negative waits above. Allow ordinary loaded-CI startup latency.
        let failure = Process::launch_supervised(command, Instant::now() + Duration::from_secs(1))
            .unwrap_err();
        let crate::process::LaunchFailure::Started { source, process } = failure else {
            panic!("expected a started helper");
        };
        assert_eq!(source.kind(), std::io::ErrorKind::TimedOut);
        process.terminate_until(ChildFixture::deadline()).unwrap();
    }
}
