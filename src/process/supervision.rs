// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ProcessObservation, identity::IdentitySocket};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    net::{AddressFamily, RecvFlags, SocketFlags, SocketType, recv, socketpair},
};
use std::{io, process::Stdio, time::Instant};

mod startup;
pub(super) use startup::StartupWatch;
mod guardian;
pub use guardian::{GuardianWatch, ShutdownTrigger};
mod admission;
pub use admission::Admitted;
mod registration;
pub use registration::Registered;

mod private {
    pub trait Sealed {}
}
pub trait SupervisionState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub struct Unpaired;
#[derive(Debug)]
pub struct Paired;
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
/// Mutual stable-identity observation between trusted supervisor/guardian code.
/// Pairing is not readiness, registration, containment or command admission.
/// Neither side may expose this endpoint or its process identity to commands.
///
/// ```compile_fail
/// use conglobate::process::{Supervision, Unpaired};
/// fn cannot_observe_before_exchange(channel: Supervision<Unpaired>) {
///     channel.peer_lost();
/// }
/// ```
#[derive(Debug)]
pub struct Supervision<S: SupervisionState> {
    socket: ControlSocket,
    peer: S::Resources,
}
/// Startup endpoint transferred exclusively to a trusted helper's stdin.
#[derive(Debug)]
pub struct SupervisionChild(OwnedFd);
impl From<SupervisionChild> for Stdio {
    fn from(child: SupervisionChild) -> Self {
        child.0.into()
    }
}
impl Supervision<Unpaired> {
    pub fn pair() -> io::Result<(Self, SupervisionChild)> {
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
    pub fn from_stdin() -> io::Result<Self> {
        Ok(Self {
            socket: ControlSocket(IdentitySocket::from_stdin()?),
            peer: (),
        })
    }
    /// Both peers send their own pidfd, then receive the other under one deadline.
    /// Error closes/cancels the channel; separately owned children still need reaping.
    pub fn exchange_until(self, deadline: Instant) -> io::Result<Supervision<Paired>> {
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
pub trait ObservedState: SupervisionState<Resources = ProcessObservation> {}
impl ObservedState for Paired {}
impl ObservedState for Registered {}
impl ObservedState for Admitted {}
impl<S: ObservedState> Supervision<S> {
    pub(super) fn duplicate_observation(&self) -> io::Result<ProcessObservation> {
        Ok(ProcessObservation {
            identity: self.peer.identity.try_clone()?,
        })
    }

    /// Observe actual peer exit, independently of channel shutdown. Use after
    /// expected channel completion to avoid killing a helper during its normal
    /// exit. A timeout still requires cancellation; success does not reap it or
    /// establish descendant quiescence.
    pub fn wait_for_peer_exit_until(&self, deadline: Instant) -> io::Result<()> {
        self.peer.wait_until(deadline)
    }

    /// Wait on both the stable process identity and the private channel. Timeout
    /// is not evidence of liveness; no resource cleanup or reaping happens here.
    pub fn wait_for_loss_until(&self, deadline: Instant) -> io::Result<()> {
        loop {
            if self.peer_lost()? {
                return Ok(());
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "supervision wait deadline expired",
                ));
            }
            let timeout = Timespec {
                tv_sec: remaining
                    .as_secs()
                    .try_into()
                    .map_err(|_| io::Error::other("supervision deadline too distant"))?,
                tv_nsec: remaining.subsec_nanos().into(),
            };
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
    /// Outside an explicit registration exchange, no messages are accepted. Invalid
    /// traffic remains queued so subsequent observations cannot report healthy.
    pub fn peer_lost(&self) -> io::Result<bool> {
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
    fn setup_exchange_observes_upstream_loss_and_protocol_failure() {
        let (supervisor, upstream) = Fixture::paired();
        let (pending, held_child) = Supervision::pair().unwrap();
        let closer = std::thread::spawn(move || {
            // Wait for the actual setup request, then lose upstream without
            // replying. Scheduler delays cannot establish this ordering.
            let socket = IdentitySocket(held_child.0);
            let _peer = socket.receive_until(Fixture::deadline()).unwrap();
            drop(upstream);
        });
        let error = pending
            .exchange_with(&StartupWatch::new(&supervisor, Fixture::deadline()))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::ConnectionAborted);
        closer.join().unwrap();
        let failure = crate::process::Process::launch_watched(
            Command::new("/nonexistent-watched-helper"),
            &supervisor,
            Fixture::deadline(),
        )
        .unwrap_err();
        assert!(
            matches!(failure, crate::process::LaunchFailure::Before(source)
            if source.kind() == io::ErrorKind::ConnectionAborted)
        );
        let (supervisor, upstream) = Fixture::paired();
        send(&upstream.socket.0.0, b"invalid", SendFlags::NOSIGNAL).unwrap();
        let (pending, _held_child) = Supervision::pair().unwrap();
        assert_eq!(
            pending
                .exchange_with(&StartupWatch::new(&supervisor, Fixture::deadline()))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            supervisor.peer_lost().unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn guardian_observes_either_peer_and_keeps_protocol_errors() {
        // Test-only state construction isolates the four-FD observation logic.
        // The disposable VM separately exercises actual helper registration.
        for supervisor_first in [true, false] {
            let (supervisor, upstream) = Fixture::paired();
            let (helper, downstream) = Fixture::paired();
            let helper = Supervision::<Registered> {
                socket: helper.socket,
                peer: helper.peer,
            };
            let watch = GuardianWatch::new(&supervisor, &helper);
            assert_eq!(
                watch
                    .wait_until(Instant::now() + Duration::from_millis(10))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::TimedOut
            );
            if supervisor_first {
                drop(upstream);
                assert_eq!(
                    watch.wait_until(Fixture::deadline()).unwrap(),
                    ShutdownTrigger::SupervisorLost
                );
                drop(downstream);
            } else {
                drop(downstream);
                assert_eq!(
                    watch.wait_until(Fixture::deadline()).unwrap(),
                    ShutdownTrigger::HelperLost
                );
                drop(upstream);
            }
            assert_eq!(
                watch.wait_until(Fixture::deadline()).unwrap(),
                ShutdownTrigger::BothLost
            );
        }
        let (supervisor, _upstream) = Fixture::paired();
        let (helper, downstream) = Fixture::paired();
        let helper = Supervision::<Registered> {
            socket: helper.socket,
            peer: helper.peer,
        };
        send(&downstream.socket.0.0, b"invalid", SendFlags::NOSIGNAL).unwrap();
        let watch = GuardianWatch::new(&supervisor, &helper);
        assert_eq!(
            watch.wait_until(Fixture::deadline()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
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
        let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
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
}
