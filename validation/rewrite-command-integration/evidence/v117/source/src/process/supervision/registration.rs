// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Paired, ProcessObservation, Supervision, SupervisionState, private};
use crate::{
    domain::DomainRegistration,
    process::identity::{ExchangeWait, IdentitySocket},
};
use rustix::{
    event::PollFlags,
    fd::{AsFd, OwnedFd},
    net::{
        RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
        SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
    },
};
use std::{
    fs::{self, File},
    io::{self, IoSlice, IoSliceMut, Write},
    mem::MaybeUninit,
    time::Instant,
};

/// Trusted helper acknowledged self-placement in a command domain. This is not
/// sandbox readiness, persisted recovery ownership or permission to run a recipe.
#[derive(Debug)]
pub struct Registered;
impl private::Sealed for Registered {}
impl SupervisionState for Registered {
    type Resources = ProcessObservation;
}

impl Supervision<Paired> {
    /// Supervisor side. Domain ownership stays with the caller on every outcome.
    /// A verified persisted-domain registration capability is required. Retain
    /// the separate child wait owner. Errors cancel the channel; they do not remove the domain or reap.
    pub fn register_helper_until(
        self,
        domain: DomainRegistration<'_>,
        deadline: Instant,
    ) -> io::Result<Supervision<Registered>> {
        self.register_helper_with(domain, &deadline)
    }
    /// Observe upstream loss during descriptor transfer and registration ACK.
    pub fn register_helper_watched(
        self,
        domain: DomainRegistration<'_>,
        supervisor: &Supervision<Paired>,
        deadline: Instant,
    ) -> io::Result<Supervision<Registered>> {
        self.register_helper_with(domain, &super::StartupWatch::new(supervisor, deadline))
    }
    fn register_helper_with(
        self,
        domain: DomainRegistration<'_>,
        wait: &impl ExchangeWait,
    ) -> io::Result<Supervision<Registered>> {
        Packet::Offer(domain.registration_file()?.into()).send_with(&self.socket.0, wait)?;
        match Packet::receive_with(&self.socket.0, wait)? {
            Packet::Joined => Ok(Supervision {
                socket: self.socket,
                peer: self.peer,
            }),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected domain registration acknowledgment",
            )),
        }
    }
    /// Helper side, before creating any threads or descendants. The helper writes
    /// its own membership, closes the transferred control FD, then acknowledges.
    /// The task-count check diagnoses misuse; trusted startup must prevent races.
    pub fn join_domain_until(self, deadline: Instant) -> io::Result<Supervision<Registered>> {
        if fs::read_dir("/proc/self/task")?
            .collect::<Result<Vec<_>, _>>()?
            .len()
            != 1
            || rustix::process::getpid() == rustix::process::Pid::INIT
        {
            return Err(io::Error::other(
                "domain registration requires a dedicated single-threaded helper",
            ));
        }
        let Packet::Offer(descriptor) = Packet::receive(&self.socket.0, deadline)? else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "expected domain registration descriptor",
            ));
        };
        if rustix::fs::fstatfs(&descriptor)?.f_type as u64 != 0x6367_7270 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "registration descriptor is not cgroup2",
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
    Admit,
    Admitted,
    Volume(OwnedFd, OwnedFd),
    Prepared,
}
impl Packet {
    // Canonical CBOR [version=1, opcode]. Offer=1, Joined=2, Admit=3, Admitted=4, Volume=5, Prepared=6.
    pub(super) fn send(self, socket: &IdentitySocket, deadline: Instant) -> io::Result<()> {
        self.send_with(socket, &deadline)
    }
    fn send_with(self, socket: &IdentitySocket, wait: &impl ExchangeWait) -> io::Result<()> {
        let (opcode, descriptors) = match &self {
            Self::Offer(fd) => (1, vec![fd.as_fd()]),
            Self::Joined => (2, vec![]),
            Self::Admit => (3, vec![]),
            Self::Admitted => (4, vec![]),
            Self::Volume(root, namespace) => (5, vec![root.as_fd(), namespace.as_fd()]),
            Self::Prepared => (6, vec![]),
        };
        loop {
            wait.ready(socket, PollFlags::OUT)?;
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if !descriptors.is_empty()
                && !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors))
            {
                return Err(io::Error::other("registration ancillary buffer too small"));
            }
            match sendmsg(
                &socket.0,
                &[IoSlice::new(&[0x82, 1, opcode])],
                &mut ancillary,
                SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
            ) {
                Ok(3) => return Ok(()),
                Ok(_) => return Err(io::Error::other("incomplete registration packet")),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
    pub(super) fn receive(socket: &IdentitySocket, deadline: Instant) -> io::Result<Self> {
        Self::receive_with(socket, &deadline)
    }
    fn receive_with(socket: &IdentitySocket, wait: &impl ExchangeWait) -> io::Result<Self> {
        loop {
            wait.ready(socket, PollFlags::IN)?;
            let mut bytes = [0; 3];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
            let mut ancillary = RecvAncillaryBuffer::new(&mut space);
            let result = match recvmsg(
                &socket.0,
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
                    "registration peer closed",
                ));
            }
            if result.bytes != 3
                || result
                    .flags
                    .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                || bytes[..2] != [0x82, 1]
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid registration packet",
                ));
            }
            let mut descriptors = Vec::new();
            for message in ancillary.drain() {
                let RecvAncillaryMessage::ScmRights(rights) = message else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unexpected registration control",
                    ));
                };
                descriptors.extend(rights);
            }
            return match (bytes[2], descriptors.len()) {
                (1, 1) => Ok(Self::Offer(descriptors.pop().expect("one descriptor"))),
                (2, 0) => Ok(Self::Joined),
                (3, 0) => Ok(Self::Admit),
                (4, 0) => Ok(Self::Admitted),
                (6, 0) => Ok(Self::Prepared),
                (5, 2) => {
                    let namespace = descriptors.pop().expect("namespace descriptor");
                    Ok(Self::Volume(
                        descriptors.pop().expect("root descriptor"),
                        namespace,
                    ))
                }
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "registration opcode/descriptor mismatch",
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
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
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
    fn registration_packets_require_exact_opcode_descriptor_pairs() {
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
    fn registration_packets_transfer_cloexec_ownership_and_acknowledge() {
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
    fn registration_receive_is_bounded_and_rejects_peer_loss() {
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
