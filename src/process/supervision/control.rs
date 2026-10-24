// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Joined, Supervision, joining::Packet};
use crate::isolation::ExecutionEndpoint;
use std::{io, os::unix::process::ExitStatusExt, process::ExitStatus, time::Instant};

impl Supervision<Joined> {
    pub(crate) fn send_gate_until(
        &self,
        endpoint: ExecutionEndpoint,
        deadline: Instant,
    ) -> io::Result<()> {
        Packet::Gate(endpoint.into_handle()).send(&self.socket.0, deadline)
    }
    pub(crate) fn receive_gate_until(&self, deadline: Instant) -> io::Result<ExecutionEndpoint> {
        match Packet::receive(&self.socket.0, deadline)? {
            Packet::Gate(fd) => ExecutionEndpoint::from_handle(fd),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected execution endpoint",
            )),
        }
    }
    /// Reports the independently reaped namespace helper after local cleanup.
    pub(crate) fn report_cleanup_until(
        &self,
        status: ExitStatus,
        deadline: Instant,
    ) -> io::Result<()> {
        Packet::Cleaned(status).send(&self.socket.0, deadline)
    }
    pub(crate) fn report_cleanup_failure_until(
        &self,
        error: &dyn std::fmt::Display,
        deadline: Instant,
    ) -> io::Result<()> {
        let mut message = format!("receiver cleanup: {error}");
        message.truncate(message.floor_char_boundary(1024));
        Packet::CleanupFailed(message).send(&self.socket.0, deadline)
    }
    pub(crate) fn cleanup_until(&self, expected: ExitStatus, deadline: Instant) -> io::Result<()> {
        match Packet::receive(&self.socket.0, deadline)? {
            Packet::Cleaned(status) => {
                let code = expected
                    .code()
                    .unwrap_or_else(|| 128 + expected.signal().unwrap_or(1));
                if status.code() == Some(code) {
                    Ok(())
                } else {
                    Err(io::Error::other(
                        "namespace helper did not finish with the reported shell outcome",
                    ))
                }
            }
            Packet::CleanupFailed(message) => Err(io::Error::other(message)),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected receiver cleanup",
            )),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::process::ExitStatusExt, time::Duration};
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
    fn direct_gate_does_not_certify_receiver_cleanup() {
        let (parent, helper) = Fixture::pair();
        let (gate, endpoint) = crate::isolation::ExecutionGate::pair().unwrap();
        parent
            .send_gate_until(endpoint, Fixture::deadline())
            .unwrap();
        let (release, held) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            helper
                .receive_gate_until(Fixture::deadline())
                .unwrap()
                .ready_for_reporting(Fixture::deadline())
                .unwrap()
                .report(ExitStatus::from_raw(37 << 8))
                .unwrap();
            held.recv().unwrap();
            helper
                .report_cleanup_until(ExitStatus::from_raw(37 << 8), Fixture::deadline())
                .unwrap();
        });
        let status = gate
            .wait_ready_until(Fixture::deadline())
            .unwrap()
            .start_reporting_until(Fixture::deadline())
            .unwrap()
            .completion_until(Fixture::deadline())
            .unwrap();
        assert_eq!(status.code(), Some(37));
        assert_eq!(
            parent
                .cleanup_until(status, Instant::now())
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        release.send(()).unwrap();
        worker.join().unwrap();
        parent.cleanup_until(status, Fixture::deadline()).unwrap();
    }
    #[test]
    fn wrong_phase_missing_receipt_and_explicit_failure_reject() {
        let (parent, helper) = Fixture::pair();
        Packet::Joined
            .send(&helper.socket.0, Fixture::deadline())
            .unwrap();
        assert_eq!(
            parent
                .cleanup_until(ExitStatus::from_raw(0), Fixture::deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        helper
            .report_cleanup_failure_until(
                &"resolver scaffold entry was replaced",
                Fixture::deadline(),
            )
            .unwrap();
        drop(helper);
        assert_eq!(
            parent
                .cleanup_until(ExitStatus::from_raw(0), Fixture::deadline())
                .unwrap_err()
                .to_string(),
            "receiver cleanup: resolver scaffold entry was replaced"
        );
        assert!(
            parent
                .cleanup_until(ExitStatus::from_raw(0), Fixture::deadline())
                .is_err()
        );
    }
    #[test]
    fn cleanup_requires_independent_helper_status_to_agree_with_shell() {
        for (shell, helper, accepted) in [
            (0, 0, true),
            (37 << 8, 37 << 8, true),
            (9, 137 << 8, true),
            (11 | 0x80, 139 << 8, true),
            (0, 37 << 8, false),
            (9, 9, false),
        ] {
            let (parent, receiver) = Fixture::pair();
            receiver
                .report_cleanup_until(ExitStatus::from_raw(helper), Fixture::deadline())
                .unwrap();
            assert_eq!(
                parent
                    .cleanup_until(ExitStatus::from_raw(shell), Fixture::deadline())
                    .is_ok(),
                accepted
            );
        }
    }
    #[test]
    fn endpoint_transfer_rejects_wrong_phase_and_descriptor_type() {
        let (parent, helper) = Fixture::pair();
        assert_eq!(
            parent
                .receive_gate_until(Instant::now())
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        Packet::Joined
            .send(&helper.socket.0, Fixture::deadline())
            .unwrap();
        assert_eq!(
            parent
                .receive_gate_until(Fixture::deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        let (fd, _) = std::os::unix::net::UnixDatagram::pair().unwrap();
        Packet::Gate(fd.into())
            .send(&helper.socket.0, Fixture::deadline())
            .unwrap();
        assert_eq!(
            parent
                .receive_gate_until(Fixture::deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
}
