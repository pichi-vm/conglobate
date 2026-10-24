// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! One-shot identity handoff between trusted processes before command admission.
//! A private socketpair is the authority; no pathname or numeric PID is accepted.
use super::ProcessObservation;
#[cfg(test)]
use rustix::net::{AddressFamily, SocketFlags, socketpair};
#[cfg(test)]
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
use rustix::{
    event::{PollFd, PollFlags, poll},
    fd::{AsFd, OwnedFd},
    net::SocketType,
    process::{PidfdFlags, getpid, pidfd_open},
};
use std::{io, time::Instant};
#[cfg(test)]
use std::{io::IoSlice, mem::MaybeUninit};

#[derive(Debug)]
pub(super) struct IdentitySocket(pub(super) OwnedFd);
pub(super) trait ExchangeWait {
    fn ready(&self, socket: &IdentitySocket, interest: PollFlags) -> io::Result<()>;
}
impl ExchangeWait for Instant {
    fn ready(&self, socket: &IdentitySocket, interest: PollFlags) -> io::Result<()> {
        socket.ready(interest, *self)
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

    #[cfg(test)]
    pub(super) fn send_current_until(&self, deadline: Instant) -> io::Result<()> {
        self.send_current_with(&deadline)
    }
    pub(super) fn send_current_with(&self, wait: &impl ExchangeWait) -> io::Result<()> {
        let identity = pidfd_open(getpid(), PidfdFlags::empty())?;
        loop {
            wait.ready(self, PollFlags::OUT)?;
            let descriptors = [identity.as_fd()];
            match crate::packet::DescriptorPacket::send_now(
                &self.0,
                &IdentitySocket::OFFER,
                &descriptors,
            ) {
                Ok(()) => return Ok(()),
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            }
        }
    }

    #[cfg(test)]
    pub(super) fn receive_until(&self, deadline: Instant) -> io::Result<ProcessObservation> {
        self.receive_with(&deadline)
    }
    pub(super) fn receive_with(&self, wait: &impl ExchangeWait) -> io::Result<ProcessObservation> {
        loop {
            wait.ready(self, PollFlags::IN)?;
            let mut bytes = [0; 3];
            let result = match crate::packet::DescriptorPacket::receive_now(&self.0, &mut bytes) {
                Ok(result) => result,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };
            if result.bytes == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "identity peer closed",
                ));
            }
            if result.bytes != bytes.len()
                || bytes != IdentitySocket::OFFER
                || result.descriptors.len() != 1
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid identity offer",
                ));
            }
            return Ok(ProcessObservation {
                identity: result.descriptors.into_iter().next().expect("one identity"),
            });
        }
    }

    // Deterministic CBOR [protocol version 1, identity-offer opcode 0].
    // SCM_RIGHTS contains exactly one self-pidfd. No extensible payload yet.
    const OFFER: [u8; 3] = [0x82, 0x01, 0x00];

    pub(super) fn ready(&self, interest: PollFlags, deadline: Instant) -> io::Result<()> {
        loop {
            let timeout = crate::deadline::Deadline::at(deadline)
                .remaining("identity exchange deadline expired")?;
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
        fn pair() -> (IdentitySocket, IdentitySocket) {
            let (a, b) = socketpair(
                AddressFamily::UNIX,
                SocketType::SEQPACKET,
                SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
                None,
            )
            .unwrap();
            (IdentitySocket(a), IdentitySocket(b))
        }
        fn invalid_offer(bytes: &[u8], descriptor_count: usize) {
            let (receiver, sender) = Self::pair();
            let identity = pidfd_open(getpid(), PidfdFlags::empty()).unwrap();
            let descriptors = vec![identity.as_fd(); descriptor_count];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if descriptor_count != 0 {
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)));
            }
            sendmsg(
                &sender.0,
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
    fn transferred_identity_is_close_on_exec_and_observes_live_sender() {
        let (receiver, sender) = Fixture::pair();
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
