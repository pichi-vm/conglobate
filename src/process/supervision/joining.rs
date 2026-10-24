// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Paired, ProcessObservation, Supervision, SupervisionState, private};
use crate::{
    domain::DomainJoin,
    process::identity::{ExchangeWait, IdentitySocket},
};
#[cfg(test)]
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
use rustix::{
    event::PollFlags,
    fd::{AsFd, OwnedFd},
};
use std::{
    fs::File,
    io::{self, Write},
    os::unix::process::ExitStatusExt,
    process::ExitStatus,
    time::Instant,
};
#[cfg(test)]
use std::{io::IoSlice, mem::MaybeUninit};

/// Trusted helper acknowledged self-placement in a command domain. This is not
/// sandbox readiness, persisted recovery ownership or permission to run a recipe.
#[derive(Debug)]
pub(crate) struct Joined;
impl private::Sealed for Joined {}
impl SupervisionState for Joined {
    type Resources = ProcessObservation;
}

impl Supervision<Paired> {
    /// Supervisor side. Domain ownership stays with the caller on every outcome.
    /// A verified persisted-domain joining capability is required. Retain
    /// the separate child wait owner. Errors cancel the channel; they do not remove the domain or reap.
    pub(crate) fn join_helper_until(
        self,
        domain: DomainJoin<'_>,
        deadline: Instant,
    ) -> io::Result<Supervision<Joined>> {
        self.join_helper_with(domain, &deadline)
    }
    fn join_helper_with(
        self,
        domain: DomainJoin<'_>,
        wait: &impl ExchangeWait,
    ) -> io::Result<Supervision<Joined>> {
        Packet::Offer(domain.join_file()?.into()).send_with(&self.socket.0, wait)?;
        match Packet::receive_with(&self.socket.0, wait)? {
            Packet::Joined => Ok(Supervision {
                socket: self.socket,
                peer: self.peer,
            }),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected domain joining acknowledgment",
            )),
        }
    }
    /// Helper side, before creating any threads or descendants. The helper writes
    /// its own membership, closes the transferred control FD, then acknowledges.
    /// The task-count check diagnoses misuse; trusted startup must prevent races.
    pub(crate) fn join_domain_until(self, deadline: Instant) -> io::Result<Supervision<Joined>> {
        if !crate::process::ThreadSnapshot::capture()?.is_single()
            || rustix::process::getpid() == rustix::process::Pid::INIT
        {
            return Err(io::Error::other(
                "domain joining requires a dedicated single-threaded helper",
            ));
        }
        let Packet::Offer(descriptor) = Packet::receive(&self.socket.0, deadline)? else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected domain joining descriptor",
            ));
        };
        if rustix::fs::fstatfs(&descriptor)?.f_type as u64 != 0x6367_7270 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "joining descriptor is not cgroup2",
            ));
        }
        let mut file = File::from(descriptor);
        file.write_all(b"0")?;
        drop(file);
        Packet::Joined.send(&self.socket.0, deadline)?;
        Ok(Supervision {
            socket: self.socket,
            peer: self.peer,
        })
    }
}

#[derive(Debug)]
pub(super) enum Packet {
    Offer(OwnedFd),
    Joined,
    Gate(OwnedFd),
    Volume(OwnedFd, OwnedFd),
    Bootstrap(OwnedFd, OwnedFd, OwnedFd),
    VolumeNetwork(OwnedFd, OwnedFd, OwnedFd, OwnedFd, OwnedFd),
    BootstrapNetwork(OwnedFd, OwnedFd, OwnedFd, OwnedFd, OwnedFd, OwnedFd),
    Cleaned(ExitStatus),
    CleanupFailed(String),
}
impl Packet {
    // Canonical CBOR [version=1, opcode]. Offer=1, Joined=2, Volume=5; Cleaned=[1,11,bstr(wait status)]; Bootstrap=8 (tool, output, namespace).
    // VolumeNetwork=9 (root, user, network, resolver, raw cleanup root);
    // BootstrapNetwork=10 adds output immediately after root; Cleaned=11.
    // CleanupFailed=[1,12,text], at most 1024 UTF-8 bytes, no descriptors. Gate=13 transfers the init endpoint.
    pub(super) fn send(self, socket: &IdentitySocket, deadline: Instant) -> io::Result<()> {
        self.send_with(socket, &deadline)
    }
    fn send_with(self, socket: &IdentitySocket, wait: &impl ExchangeWait) -> io::Result<()> {
        let (opcode, descriptors) = match &self {
            Self::Offer(fd) => (1, vec![fd.as_fd()]),
            Self::Gate(fd) => (13, vec![fd.as_fd()]),
            Self::Joined => (2, vec![]),
            Self::Volume(root, namespace) => (5, vec![root.as_fd(), namespace.as_fd()]),
            Self::Bootstrap(root, output, namespace) => {
                (8, vec![root.as_fd(), output.as_fd(), namespace.as_fd()])
            }
            Self::VolumeNetwork(root, user, network, resolver, raw) => (
                9,
                vec![
                    root.as_fd(),
                    user.as_fd(),
                    network.as_fd(),
                    resolver.as_fd(),
                    raw.as_fd(),
                ],
            ),
            Self::BootstrapNetwork(root, output, user, network, resolver, raw) => (
                10,
                vec![
                    root.as_fd(),
                    output.as_fd(),
                    user.as_fd(),
                    network.as_fd(),
                    resolver.as_fd(),
                    raw.as_fd(),
                ],
            ),
            Self::CleanupFailed(_) => (12, vec![]),
            Self::Cleaned(_) => (11, vec![]),
        };
        // The status payload is a fixed four-byte Linux wait status, carried
        // in a canonical CBOR byte string rather than a native-endian integer.
        let bytes = match &self {
            Self::CleanupFailed(message) => {
                if message.is_empty() || message.len() > 1024 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid cleanup diagnostic size",
                    ));
                }
                let mut encoded = minicbor::Encoder::new(Vec::new());
                encoded
                    .array(3)
                    .and_then(|e| e.u8(1))
                    .and_then(|e| e.u8(12))
                    .and_then(|e| e.str(message))
                    .expect("Vec writer");
                encoded.into_writer()
            }
            Self::Cleaned(status) => {
                let mut bytes = vec![0x83, 1, opcode, 0x44];
                bytes.extend_from_slice(&status.into_raw().to_be_bytes());
                bytes
            }
            _ => vec![0x82, 1, opcode],
        };
        loop {
            wait.ready(socket, PollFlags::OUT)?;
            match crate::packet::DescriptorPacket::send_now(&socket.0, &bytes, &descriptors) {
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
    pub(super) fn receive(socket: &IdentitySocket, deadline: Instant) -> io::Result<Self> {
        Self::receive_with(socket, &deadline)
    }
    fn receive_with(socket: &IdentitySocket, wait: &impl ExchangeWait) -> io::Result<Self> {
        loop {
            wait.ready(socket, PollFlags::IN)?;
            let mut bytes = [0; 1030];
            let result = match crate::packet::DescriptorPacket::receive_now(&socket.0, &mut bytes) {
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
                    "joining peer closed",
                ));
            }
            if !((result.bytes == 3 && bytes[..2] == [0x82, 1] && bytes[2] != 11)
                || (result.bytes == 8 && bytes[..4] == [0x83, 1, 11, 0x44])
                || (result.bytes >= 5 && bytes[..3] == [0x83, 1, 12]))
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid joining packet",
                ));
            }
            let mut descriptors = result.descriptors;
            return match (bytes[2], descriptors.len()) {
                (1, 1) => Ok(Self::Offer(descriptors.pop().expect("one descriptor"))),
                (13, 1) => Ok(Self::Gate(descriptors.pop().expect("gate"))),
                (2, 0) => Ok(Self::Joined),
                (12, 0) => {
                    let mut decoder = minicbor::Decoder::new(&bytes[3..result.bytes]);
                    let message = decoder.str().map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "invalid cleanup diagnostic")
                    })?;
                    let mut canonical = minicbor::Encoder::new(Vec::new());
                    canonical.str(message).expect("Vec writer");
                    if message.is_empty()
                        || message.len() > 1024
                        || canonical.writer().as_slice() != &bytes[3..result.bytes]
                    {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "invalid cleanup diagnostic encoding or size",
                        ));
                    }
                    Ok(Self::CleanupFailed(message.to_owned()))
                }
                (11, 0) => {
                    // Framing above excludes opcode 11 from the three-byte form.
                    // This arm therefore has exactly eight bytes and no rights.
                    let raw = i32::from_be_bytes(bytes[4..8].try_into().expect("four bytes"));
                    let status = ExitStatus::from_raw(raw);
                    // Accept only terminal Linux wait statuses, not stopped or
                    // continued children, reserved bits or impossible signals.
                    let valid = (raw & !0xff00 == 0)
                        || (raw > 0 && raw & !0xff == 0 && (1..=64).contains(&(raw & 0x7f)));
                    if !valid {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "nonterminal or invalid command status",
                        ));
                    }
                    Ok(Self::Cleaned(status))
                }
                (9, 5) => {
                    let raw = descriptors.pop().expect("raw cleanup root");
                    let resolver = descriptors.pop().expect("resolver");
                    let network = descriptors.pop().expect("network");
                    let user = descriptors.pop().expect("user");
                    Ok(Self::VolumeNetwork(
                        descriptors.pop().expect("root"),
                        user,
                        network,
                        resolver,
                        raw,
                    ))
                }
                (10, 6) => {
                    let raw = descriptors.pop().expect("raw cleanup root");
                    let resolver = descriptors.pop().expect("resolver");
                    let network = descriptors.pop().expect("network");
                    let user = descriptors.pop().expect("user");
                    let output = descriptors.pop().expect("output");
                    Ok(Self::BootstrapNetwork(
                        descriptors.pop().expect("root"),
                        output,
                        user,
                        network,
                        resolver,
                        raw,
                    ))
                }
                (8, 3) => {
                    let namespace = descriptors.pop().expect("namespace descriptor");
                    let output = descriptors.pop().expect("output descriptor");
                    Ok(Self::Bootstrap(
                        descriptors.pop().expect("tool descriptor"),
                        output,
                        namespace,
                    ))
                }
                (5, 2) => {
                    let namespace = descriptors.pop().expect("namespace descriptor");
                    Ok(Self::Volume(
                        descriptors.pop().expect("root descriptor"),
                        namespace,
                    ))
                }
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "joining opcode/descriptor mismatch",
                )),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustix::{
        fd::BorrowedFd,
        net::{AddressFamily, SocketFlags, SocketType, socketpair},
    };
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
        fn deadline() -> Instant {
            Instant::now() + Duration::from_secs(2)
        }
        fn raw(socket: &IdentitySocket, bytes: &[u8], fds: &[BorrowedFd<'_>]) {
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(7))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if !fds.is_empty() {
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(fds)));
            }
            sendmsg(
                &socket.0,
                &[IoSlice::new(bytes)],
                &mut ancillary,
                SendFlags::NOSIGNAL,
            )
            .unwrap();
        }
    }
    #[test]
    fn cleanup_failure_is_bounded_canonical_and_descriptor_free() {
        for message in ["cause".to_owned(), "é".repeat(512)] {
            let (a, b) = Fixture::pair();
            Packet::CleanupFailed(message.clone())
                .send(&a, Fixture::deadline())
                .unwrap();
            assert!(matches!(Packet::receive(&b, Fixture::deadline()).unwrap(),
                Packet::CleanupFailed(actual) if actual == message));
        }
        for message in [String::new(), "x".repeat(1025)] {
            let (a, _) = Fixture::pair();
            assert_eq!(
                Packet::CleanupFailed(message)
                    .send(&a, Fixture::deadline())
                    .unwrap_err()
                    .kind(),
                io::ErrorKind::InvalidInput
            );
        }
        for bytes in [
            vec![0x83, 1, 12, 0x60],
            vec![0x83, 1, 12, 0x61, 0xff],
            vec![0x83, 1, 12, 0x78, 1, b'x'],
            vec![0x83, 1, 12, 0x61, b'x', 0],
            vec![0x82, 1, 12],
        ] {
            let (a, b) = Fixture::pair();
            Fixture::raw(&a, &bytes, &[]);
            assert_eq!(
                Packet::receive(&b, Fixture::deadline()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        let (a, b) = Fixture::pair();
        let file = std::fs::File::open("/dev/null").unwrap();
        Fixture::raw(&a, &[0x83, 1, 12, 0x61, b'x'], &[file.as_fd()]);
        assert!(Packet::receive(&b, Fixture::deadline()).is_err());
    }
    #[test]
    fn completion_preserves_exit_and_signal_status() {
        for raw in [0, 37 << 8, 255 << 8, 9, 11 | 0x80] {
            let (a, b) = Fixture::pair();
            Packet::Cleaned(ExitStatus::from_raw(raw))
                .send(&a, Fixture::deadline())
                .unwrap();
            let Packet::Cleaned(status) = Packet::receive(&b, Fixture::deadline()).unwrap() else {
                panic!("expected completion");
            };
            assert_eq!(status.into_raw(), raw);
        }
    }
    #[test]
    fn completion_rejects_nonterminal_status_and_unexpected_rights() {
        let file = File::open("/dev/null").unwrap();
        for raw in [-1i32, 0xffff, 0x137f, 0x10000, 65, 0x80] {
            let (a, b) = Fixture::pair();
            Packet::Cleaned(ExitStatus::from_raw(raw))
                .send(&a, Fixture::deadline())
                .unwrap();
            assert_eq!(
                Packet::receive(&b, Fixture::deadline()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        for bytes in [
            &[0x82, 1, 7][..],
            &[0x83, 1, 7, 0x44, 0, 0, 0][..],
            &[0x83, 1, 7, 0x44, 0, 0, 0, 0, 0][..],
            &[0x83, 1, 7, 0x43, 0, 0, 0, 0][..],
        ] {
            let (a, b) = Fixture::pair();
            Fixture::raw(&a, bytes, &[]);
            assert_eq!(
                Packet::receive(&b, Fixture::deadline()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
        let (a, b) = Fixture::pair();
        Fixture::raw(&a, &[0x83, 1, 7, 0x44, 0, 0, 0, 0], &[file.as_fd()]);
        assert_eq!(
            Packet::receive(&b, Fixture::deadline()).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn joining_packets_require_exact_opcode_descriptor_pairs() {
        let file = File::open("/dev/null").unwrap();
        for (bytes, count) in [
            (&[0x82, 1, 1][..], 0),
            (&[0x82, 1, 2][..], 1),
            (&[0x82, 1, 1][..], 2),
            (&[0x82, 2, 2][..], 0),
            (&[0x82, 1, 9][..], 0),
            (&[0x82, 1, 3][..], 1),
            (&[0x82, 1, 4][..], 1),
            (&[0x82, 1, 5][..], 0),
            (&[0x82, 1, 5][..], 1),
            (&[0x82, 1, 6][..], 1),
            (&[0x82, 1, 8][..], 0),
            (&[0x82, 1, 8][..], 1),
            (&[0x82, 1, 8][..], 2),
            (&[0x82, 1, 2, 0][..], 0),
        ] {
            let (a, b) = Fixture::pair();
            let fds = [file.as_fd(), file.as_fd()];
            Fixture::raw(&a, bytes, &fds[..count]);
            assert_eq!(
                Packet::receive(&b, Fixture::deadline()).unwrap_err().kind(),
                io::ErrorKind::InvalidData
            );
        }
    }
    #[test]
    fn joining_packets_transfer_cloexec_ownership_and_acknowledge() {
        let (a, b) = Fixture::pair();
        Packet::Offer(File::open("/dev/null").unwrap().into())
            .send(&a, Fixture::deadline())
            .unwrap();
        let Packet::Offer(fd) = Packet::receive(&b, Fixture::deadline()).unwrap() else {
            panic!("expected offer");
        };
        assert!(
            rustix::io::fcntl_getfd(&fd)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        Packet::Joined.send(&b, Fixture::deadline()).unwrap();
        assert!(matches!(
            Packet::receive(&a, Fixture::deadline()).unwrap(),
            Packet::Joined
        ));
    }
    #[test]
    fn bootstrap_packet_preserves_three_distinct_cloexec_descriptors() {
        use std::os::unix::fs::MetadataExt;
        let (a, b) = Fixture::pair();
        let root = tempfile::tempdir().unwrap();
        let output = tempfile::tempdir().unwrap();
        let root_inode = root.path().metadata().unwrap().ino();
        let output_inode = output.path().metadata().unwrap().ino();
        Packet::Bootstrap(
            File::open(root.path()).unwrap().into(),
            File::open(output.path()).unwrap().into(),
            File::open("/proc/self/ns/user").unwrap().into(),
        )
        .send(&a, Fixture::deadline())
        .unwrap();
        let Packet::Bootstrap(root, output, namespace) =
            Packet::receive(&b, Fixture::deadline()).unwrap()
        else {
            panic!("expected bootstrap packet");
        };
        for fd in [&root, &output, &namespace] {
            assert!(
                rustix::io::fcntl_getfd(fd)
                    .unwrap()
                    .contains(rustix::io::FdFlags::CLOEXEC)
            );
        }
        assert_eq!(File::from(root).metadata().unwrap().ino(), root_inode);
        assert_eq!(File::from(output).metadata().unwrap().ino(), output_inode);
        assert_eq!(
            rustix::fs::fstatfs(namespace).unwrap().f_type as u64,
            0x6e73_6673
        );
    }
    #[test]
    fn volume_descriptors_keep_order_and_are_cloexec() {
        let (a, b) = Fixture::pair();
        Packet::Volume(
            File::open("/").unwrap().into(),
            File::open("/proc/self/ns/user").unwrap().into(),
        )
        .send(&a, Fixture::deadline())
        .unwrap();
        let Packet::Volume(root, namespace) = Packet::receive(&b, Fixture::deadline()).unwrap()
        else {
            panic!("expected volume");
        };
        for fd in [&root, &namespace] {
            assert!(
                rustix::io::fcntl_getfd(fd)
                    .unwrap()
                    .contains(rustix::io::FdFlags::CLOEXEC)
            );
        }
        assert!(File::from(root).metadata().unwrap().is_dir());
        assert_eq!(
            rustix::fs::fstatfs(namespace).unwrap().f_type as u64,
            0x6e73_6673
        );
    }
    #[test]
    fn network_packets_preserve_descriptor_order_and_cloexec() {
        for bootstrap in [false, true] {
            let (a, b) = Fixture::pair();
            let files: Vec<_> = (0..6).map(|_| tempfile::tempfile().unwrap()).collect();
            let descriptor = |i: usize| files[i].try_clone().unwrap().into();
            let packet = if bootstrap {
                Packet::BootstrapNetwork(
                    descriptor(0),
                    descriptor(1),
                    descriptor(2),
                    descriptor(3),
                    descriptor(4),
                    descriptor(5),
                )
            } else {
                Packet::VolumeNetwork(
                    descriptor(0),
                    descriptor(1),
                    descriptor(2),
                    descriptor(3),
                    descriptor(4),
                )
            };
            packet.send(&a, Fixture::deadline()).unwrap();
            let descriptors = match Packet::receive(&b, Fixture::deadline()).unwrap() {
                Packet::VolumeNetwork(a, b, c, d, e) if !bootstrap => vec![a, b, c, d, e],
                Packet::BootstrapNetwork(a, b, c, d, e, f) if bootstrap => vec![a, b, c, d, e, f],
                _ => panic!("wrong network packet"),
            };
            for (received, expected) in descriptors.iter().zip(&files) {
                assert_eq!(
                    rustix::fs::fstat(received).unwrap().st_ino,
                    rustix::fs::fstat(expected).unwrap().st_ino
                );
                assert!(
                    rustix::io::fcntl_getfd(received)
                        .unwrap()
                        .contains(rustix::io::FdFlags::CLOEXEC)
                );
            }
        }
    }
    #[test]
    fn network_packets_reject_every_incorrect_descriptor_count() {
        let file = tempfile::tempfile().unwrap();
        for (opcode, required) in [(9, 5), (10, 6)] {
            for count in (0..=7).filter(|count| *count != required) {
                let (a, b) = Fixture::pair();
                let descriptors = vec![file.as_fd(); count];
                Fixture::raw(&a, &[0x82, 1, opcode], &descriptors);
                assert_eq!(
                    Packet::receive(&b, Fixture::deadline()).unwrap_err().kind(),
                    io::ErrorKind::InvalidData
                );
            }
        }
    }
    #[test]
    fn joining_receive_is_bounded_and_rejects_peer_loss() {
        let (a, b) = Fixture::pair();
        assert_eq!(
            Packet::receive(&b, Instant::now() + Duration::from_millis(10))
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
        drop(a);
        assert_eq!(
            Packet::receive(&b, Fixture::deadline()).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
