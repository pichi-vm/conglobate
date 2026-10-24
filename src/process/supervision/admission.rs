// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Joined, ProcessObservation, Supervision, SupervisionState, joining::Packet, private};
use std::{io, time::Instant};

/// The trusted helper received the supervisor's explicit admission message.
/// This is a protocol state, not independent proof of sandbox configuration.
#[derive(Debug)]
pub(crate) struct Admitted;
impl private::Sealed for Admitted {}
impl SupervisionState for Admitted {
    type Resources = ProcessObservation;
}
impl Supervision<Joined> {
    /// Trusted helper only, after its restricted leaf reports readiness.
    pub(crate) fn report_prepared_until(&self, deadline: Instant) -> io::Result<()> {
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
    /// Dedicated trusted helper only. Wait after domain joining and before running
    /// any image command. EOF, malformed input or timeout closes the channel;
    /// none authorize execution. The supervisor separately cancels and reaps.
    pub(crate) fn await_admission_until(
        self,
        deadline: Instant,
    ) -> io::Result<Supervision<Admitted>> {
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

impl Supervision<Admitted> {
    /// Trusted receiver only, after local mounts/resolver/diagnostics are closed.
    pub(crate) fn report_cleanup_until(&self, deadline: Instant) -> io::Result<()> {
        Packet::Cleaned.send(&self.socket.0, deadline)
    }
    /// Send a bounded failure while the trusted channel is still live. This is
    /// never a cleanup receipt; the supervisor must cancel/drain and reject.
    pub(crate) fn report_cleanup_failure_until(
        &self,
        error: &dyn std::fmt::Display,
        deadline: Instant,
    ) -> io::Result<()> {
        let mut message = format!("receiver cleanup: {error}");
        message.truncate(message.floor_char_boundary(1024));
        Packet::CleanupFailed(message).send(&self.socket.0, deadline)
    }
    pub(crate) fn cleanup_until(&self, deadline: Instant) -> io::Result<()> {
        match Packet::receive(&self.socket.0, deadline)? {
            Packet::Cleaned => Ok(()),
            Packet::CleanupFailed(message) => Err(io::Error::other(message)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected receiver cleanup",
            )),
        }
    }

    /// Trusted helper reports its reaped command status, before helper cleanup.
    /// This does not certify descendant quiescence or successful finalization.
    pub(crate) fn report_completion_until(
        &self,
        status: std::process::ExitStatus,
        deadline: Instant,
    ) -> io::Result<()> {
        Packet::Completed(status).send(&self.socket.0, deadline)
    }
    pub(crate) fn completion_until(
        &self,
        deadline: Instant,
    ) -> io::Result<std::process::ExitStatus> {
        match Packet::receive(&self.socket.0, deadline)? {
            Packet::Completed(status) => Ok(status),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected command completion",
            )),
        }
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
        fn pair() -> (Supervision<Joined>, Supervision<Joined>) {
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
    fn joined_helper_waits_for_explicit_admission() {
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
    #[test]
    fn shell_completion_and_receiver_cleanup_are_separate_messages() {
        use std::os::unix::process::ExitStatusExt;
        let (parent, helper) = Fixture::pair();
        let (release, held) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let helper = helper.await_admission_until(Fixture::deadline()).unwrap();
            helper
                .report_completion_until(std::process::ExitStatus::from_raw(0), Fixture::deadline())
                .unwrap();
            held.recv().unwrap();
            helper.report_cleanup_until(Fixture::deadline()).unwrap();
            helper
        });
        let parent = parent.admit_until(Fixture::deadline()).unwrap();
        assert!(
            parent
                .completion_until(Fixture::deadline())
                .unwrap()
                .success()
        );
        assert_eq!(
            parent.cleanup_until(Instant::now()).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        release.send(()).unwrap();
        let helper = worker.join().unwrap();
        parent.cleanup_until(Fixture::deadline()).unwrap();
        drop(helper);
    }
    #[test]
    fn explicit_cleanup_failure_survives_peer_exit_and_preserves_shell_status() {
        use std::os::unix::process::ExitStatusExt;
        let (parent, helper) = Fixture::pair();
        let worker = std::thread::spawn(move || {
            let helper = helper.await_admission_until(Fixture::deadline()).unwrap();
            helper
                .report_completion_until(std::process::ExitStatus::from_raw(0), Fixture::deadline())
                .unwrap();
            helper
                .report_cleanup_failure_until(
                    &"resolver scaffold entry was replaced",
                    Fixture::deadline(),
                )
                .unwrap();
        });
        let parent = parent.admit_until(Fixture::deadline()).unwrap();
        worker.join().unwrap(); // queued packet must survive channel closure
        assert!(
            parent
                .completion_until(Fixture::deadline())
                .unwrap()
                .success()
        );
        let error = parent.cleanup_until(Fixture::deadline()).unwrap_err();
        assert_eq!(
            error.to_string(),
            "receiver cleanup: resolver scaffold entry was replaced"
        );
    }
    #[test]
    fn shell_status_cannot_substitute_for_cleanup_receipt() {
        use std::os::unix::process::ExitStatusExt;
        let (parent, helper) = Fixture::pair();
        let worker =
            std::thread::spawn(move || helper.await_admission_until(Fixture::deadline()).unwrap());
        let parent = parent.admit_until(Fixture::deadline()).unwrap();
        let helper = worker.join().unwrap();
        helper
            .report_completion_until(std::process::ExitStatus::from_raw(0), Fixture::deadline())
            .unwrap();
        assert_eq!(
            parent
                .cleanup_until(Fixture::deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        drop(helper);
        assert!(parent.cleanup_until(Fixture::deadline()).is_err());
    }
}
