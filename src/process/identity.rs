// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! One-shot identity handoff between trusted processes before command admission.
//! A private socketpair is the authority; no pathname or numeric PID is accepted.
use super::ProcessObservation;
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::{AsFd, OwnedFd},
    net::{
        AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg,
        sendmsg, socketpair,
    },
    process::{PidfdFlags, getpid, pidfd_open},
};
use std::{
    io::{self, IoSlice, IoSliceMut},
    mem::MaybeUninit,
    process::Stdio,
    time::Instant,
};

#[derive(Debug)]
pub(super) struct IdentitySocket(pub(super) OwnedFd);
/// Parent half of a private one-shot trusted helper identity exchange.
#[derive(Debug)]
pub struct IdentityReceiver(IdentitySocket);
impl Drop for IdentityReceiver {
    fn drop(&mut self) {
        // Another trusted spawn can briefly inherit CLOEXEC descriptors before
        // exec. Cancel the endpoint itself instead of waiting for every such
        // copy to close. This is normal-drop cancellation, not crash recovery.
        let _ = rustix::net::shutdown(&self.0.0, rustix::net::Shutdown::Both);
    }
}
/// Child half. Pass only to the trusted helper, never to a build command.
#[derive(Debug)]
pub struct IdentitySender(IdentitySocket);

impl IdentityReceiver {
    pub fn pair() -> io::Result<(Self, IdentitySender)> {
        let (parent, child) = socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )?;
        Ok((
            Self(IdentitySocket(parent)),
            IdentitySender(IdentitySocket(child)),
        ))
    }

    /// Receive exactly one identity from the trusted peer. Failure closes the
    /// channel; the caller must cancel/reap its separately owned helper.
    /// This is neither helper readiness nor permission to admit build commands.
    pub fn receive_until(self, deadline: Instant) -> io::Result<ProcessObservation> {
        self.0.receive_until(deadline)
    }
}

impl IdentitySender {
    /// Move the dedicated helper's inherited stdin socket into close-on-exec
    /// ownership, replacing stdin with /dev/null. Call at trusted helper startup
    /// before any concurrent stdin access. The helper must not admit
    /// commands until the later supervision and containment protocol completes.
    pub fn from_stdin() -> io::Result<Self> {
        IdentitySocket::from_stdin().map(Self)
    }

    /// Pin this process while it is necessarily alive, then transfer that
    /// identity. No parent-side PID lookup or SIGCHLD disposition assumption.
    pub fn send_current_until(self, deadline: Instant) -> io::Result<()> {
        self.0.send_current_until(deadline)
    }
}
impl From<IdentitySender> for Stdio {
    fn from(sender: IdentitySender) -> Self {
        sender.0.0.into()
    }
}
impl IdentitySocket {
    pub(super) fn from_stdin() -> io::Result<Self> {
        let socket = io::stdin().as_fd().try_clone_to_owned()?;
        if rustix::net::sockopt::socket_type(&socket)? != SocketType::SEQPACKET {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "helper stdin is not a sequence-packet socket",
            ));
        }
        rustix::stdio::dup2_stdin(std::fs::File::open("/dev/null")?)?;
        Ok(Self(socket))
    }

    pub(super) fn send_current_until(&self, deadline: Instant) -> io::Result<()> {
        let identity = pidfd_open(getpid(), PidfdFlags::empty())?;
        loop {
            self.ready(PollFlags::OUT, deadline)?;
            let descriptors = [identity.as_fd()];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
                return Err(io::Error::other("identity ancillary buffer too small"));
            }
            match sendmsg(
                &self.0,
                &[IoSlice::new(&IdentitySocket::OFFER)],
                &mut ancillary,
                SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
            ) {
                Ok(3) => return Ok(()),
                Ok(_) => return Err(io::Error::other("incomplete identity offer")),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }

    pub(super) fn receive_until(&self, deadline: Instant) -> io::Result<ProcessObservation> {
        loop {
            self.ready(PollFlags::IN, deadline)?;
            let mut bytes = [0; 3];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
            let mut ancillary = RecvAncillaryBuffer::new(&mut space);
            let result = match recvmsg(
                &self.0,
                &mut [IoSliceMut::new(&mut bytes)],
                &mut ancillary,
                RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
            ) {
                Ok(result) => result,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            };
            if result.bytes == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "identity peer closed",
                ));
            }
            if result.bytes != bytes.len()
                || bytes != IdentitySocket::OFFER
                || result
                    .flags
                    .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid identity offer",
                ));
            }
            let mut identity = None;
            for message in ancillary.drain() {
                match message {
                    RecvAncillaryMessage::ScmRights(descriptors) => {
                        for descriptor in descriptors {
                            if identity.replace(descriptor).is_some() {
                                return Err(io::Error::new(
                                    io::ErrorKind::InvalidData,
                                    "extra identity descriptor",
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "unexpected identity control message",
                        ));
                    }
                }
            }
            return identity
                .map(|identity| ProcessObservation { identity })
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "missing identity descriptor")
                });
        }
    }

    // Deterministic CBOR [protocol version 1, identity-offer opcode 0].
    // SCM_RIGHTS contains exactly one self-pidfd. No extensible payload yet.
    const OFFER: [u8; 3] = [0x82, 0x01, 0x00];

    fn ready(&self, interest: PollFlags, deadline: Instant) -> io::Result<()> {
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "identity exchange deadline expired",
                ));
            }
            let timeout = Timespec {
                tv_sec: remaining
                    .as_secs()
                    .try_into()
                    .map_err(|_| io::Error::other("identity deadline too distant"))?,
                tv_nsec: remaining.subsec_nanos().into(),
            };
            let mut fds = [PollFd::new(&self.0, interest)];
            match poll(&mut fds, Some(&timeout)) {
                Ok(0) | Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
                Ok(_) => return Ok(()), // recv/send reports EOF or socket errors.
            }
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
        fn invalid_offer(bytes: &[u8], descriptor_count: usize) {
            let (receiver, sender) = IdentityReceiver::pair().unwrap();
            let identity = pidfd_open(getpid(), PidfdFlags::empty()).unwrap();
            let descriptors = vec![identity.as_fd(); descriptor_count];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if descriptor_count != 0 {
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
            }
            sendmsg(
                &sender.0.0,
                &[IoSlice::new(bytes)],
                &mut ancillary,
                SendFlags::NOSIGNAL,
            )
            .unwrap();
            assert_eq!(
                receiver
                    .receive_until(Instant::now() + Duration::from_secs(1))
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidData
            );
        }
    }

    #[test]
    fn reject_invalid_wire_messages_and_descriptor_counts() {
        for bytes in [
            &[0x82, 2, 0][..],    // Unknown version.
            &[0x82, 1, 1][..],    // Unknown opcode.
            &[0x82, 1][..],       // Incomplete CBOR.
            &[0x82, 1, 0, 0][..], // Trailing data / truncated packet.
        ] {
            Fixture::invalid_offer(bytes, 1);
        }
        for count in [0, 2, 8] {
            Fixture::invalid_offer(&IdentitySocket::OFFER, count);
        }
    }

    #[test]
    fn receiver_cancellation_survives_a_duplicate_endpoint() {
        let (receiver, sender) = IdentityReceiver::pair().unwrap();
        let duplicate = receiver.0.0.try_clone().unwrap();
        drop(receiver);
        assert!(
            sender
                .send_current_until(Instant::now() + Duration::from_secs(1))
                .is_err()
        );
        drop(duplicate);
    }

    #[test]
    fn transferred_identity_is_close_on_exec_and_observes_live_sender() {
        let (receiver, sender) = IdentityReceiver::pair().unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        sender.send_current_until(deadline).unwrap();
        let observation = receiver.receive_until(deadline).unwrap();
        assert!(
            rustix::io::fcntl_getfd(&observation.identity)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        assert!(!observation.has_exited().unwrap());
    }
}
