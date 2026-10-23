// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{
    ProcessObservation, Registered, Supervision, SupervisionState, private, registration::Packet,
};
use std::{io, time::Instant};

/// The trusted helper received the supervisor's explicit admission message.
/// This is a protocol state, not independent proof of sandbox configuration.
#[derive(Debug)]
pub struct Admitted;
impl private::Sealed for Admitted {}
impl SupervisionState for Admitted {
    type Resources = ProcessObservation;
}
impl Supervision<Registered> {
    /// Trusted helper only, after its restricted leaf reports readiness.
    pub fn report_prepared_until(&self, deadline: Instant) -> io::Result<()> {
        Packet::Prepared.send(&self.socket.0, deadline)
    }
    pub(crate) fn wait_prepared_until(&self, deadline: Instant) -> io::Result<()> {
        if !matches!(Packet::receive(&self.socket.0, deadline)?, Packet::Prepared) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected restricted helper readiness",
            ));
        }
        Ok(())
    }

    pub(crate) fn admit_until(self, deadline: Instant) -> io::Result<Supervision<Admitted>> {
        Packet::Admit.send(&self.socket.0, deadline)?;
        if !matches!(Packet::receive(&self.socket.0, deadline)?, Packet::Admitted) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected admission acknowledgment",
            ));
        }
        Ok(Supervision {
            socket: self.socket,
            peer: self.peer,
        })
    }
    /// Dedicated trusted helper only. Wait after registration and before running
    /// any image command. EOF, malformed input or timeout closes the channel;
    /// none authorize execution. The supervisor separately cancels and reaps.
    pub fn await_admission_until(self, deadline: Instant) -> io::Result<Supervision<Admitted>> {
        if !matches!(Packet::receive(&self.socket.0, deadline)?, Packet::Admit) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected command admission",
            ));
        }
        Packet::Admitted.send(&self.socket.0, deadline)?;
        Ok(Supervision {
            socket: self.socket,
            peer: self.peer,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[derive(Debug)]
    struct Fixture;
    impl Fixture {
        fn deadline() -> Instant {
            Instant::now() + Duration::from_secs(5)
        }
        fn pair() -> (Supervision<Registered>, Supervision<Registered>) {
            let (parent, child) = Supervision::pair().unwrap();
            let thread = std::thread::spawn(move || {
                let peer = Supervision::<super::super::Unpaired> {
                    socket: super::super::ControlSocket(crate::process::identity::IdentitySocket(
                        child.0,
                    )),
                    peer: (),
                }
                .exchange_until(Self::deadline())
                .unwrap();
                Supervision {
                    socket: peer.socket,
                    peer: peer.peer,
                }
            });
            let parent = parent.exchange_until(Self::deadline()).unwrap();
            (
                Supervision {
                    socket: parent.socket,
                    peer: parent.peer,
                },
                thread.join().unwrap(),
            )
        }
    }
    #[test]
    fn prepared_notification_precedes_explicit_admission() {
        let (parent, helper) = Fixture::pair();
        let waiter = std::thread::spawn(move || {
            helper.report_prepared_until(Fixture::deadline()).unwrap();
            helper.await_admission_until(Fixture::deadline()).unwrap()
        });
        parent.wait_prepared_until(Fixture::deadline()).unwrap();
        let parent = parent.admit_until(Fixture::deadline()).unwrap();
        assert!(!waiter.join().unwrap().peer_lost().unwrap());
        drop(parent);
    }
    #[test]
    fn readiness_requires_the_exact_message_and_live_peer() {
        let (parent, helper) = Fixture::pair();
        assert_eq!(
            parent
                .wait_prepared_until(Instant::now())
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        Packet::Admitted
            .send(&helper.socket.0, Fixture::deadline())
            .unwrap();
        assert_eq!(
            parent
                .wait_prepared_until(Fixture::deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        drop(helper);
        assert!(parent.wait_prepared_until(Fixture::deadline()).is_err());
    }
    #[test]
    fn registered_helper_waits_for_explicit_admission() {
        let (parent, helper) = Fixture::pair();
        let waiter =
            std::thread::spawn(move || helper.await_admission_until(Fixture::deadline()).unwrap());
        let parent = parent.admit_until(Fixture::deadline()).unwrap();
        let helper = waiter.join().unwrap();
        assert!(!parent.peer_lost().unwrap());
        assert!(!helper.peer_lost().unwrap());
    }
    #[test]
    fn no_admission_timeout_and_wrong_phase_fail_closed() {
        let (parent, helper) = Fixture::pair();
        assert_eq!(
            helper
                .await_admission_until(Instant::now())
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        assert!(parent.peer_lost().unwrap());
        let (parent, helper) = Fixture::pair();
        Packet::Joined
            .send(&parent.socket.0, Fixture::deadline())
            .unwrap();
        assert_eq!(
            helper
                .await_admission_until(Fixture::deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert!(parent.peer_lost().unwrap());
    }
}
