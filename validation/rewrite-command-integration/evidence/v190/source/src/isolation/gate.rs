// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    net::{
        self, AddressFamily, RecvAncillaryBuffer, RecvFlags, ReturnFlags, SendFlags, SocketFlags,
        SocketType,
    },
};
use std::{
    io::{self, IoSliceMut},
    marker::PhantomData,
    mem::MaybeUninit,
    time::Instant,
};

mod private {
    pub trait Sealed {}
}
pub trait GateState: private::Sealed + std::fmt::Debug {}
#[derive(Debug)]
pub struct Waiting;
#[derive(Debug)]
pub struct Ready;
impl private::Sealed for Waiting {}
impl private::Sealed for Ready {}
impl GateState for Waiting {}
impl GateState for Ready {}

#[derive(Debug)]
struct Control(Socket);
impl Drop for Control {
    fn drop(&mut self) {
        let _ = net::shutdown(&self.0.0, net::Shutdown::Both);
    }
}
/// Trusted parent only. Readiness is distinct from authorization to start.
/// ```compile_fail
/// use conglobate::isolation::{ExecutionGate, Waiting};
/// fn cannot_start_before_ready(gate: ExecutionGate<Waiting>) {
///     gate.start_until(std::time::Instant::now());
/// }
/// ```
#[derive(Debug)]
pub struct ExecutionGate<S: GateState> {
    socket: Control,
    state: PhantomData<S>,
}
/// Passed only between trusted setup helpers, then closed before image spawn.
#[derive(Debug)]
pub struct ExecutionEndpoint(Socket);
#[derive(Debug)]
struct Socket(OwnedFd);
impl ExecutionGate<Waiting> {
    pub fn pair() -> io::Result<(Self, ExecutionEndpoint)> {
        let (parent, child) = net::socketpair(
            AddressFamily::UNIX,
            SocketType::SEQPACKET,
            SocketFlags::CLOEXEC | SocketFlags::NONBLOCK,
            None,
        )?;
        Ok((
            Self {
                socket: Control(Socket(parent)),
                state: PhantomData,
            },
            ExecutionEndpoint(Socket(child)),
        ))
    }
    pub fn wait_ready_until(self, deadline: Instant) -> io::Result<ExecutionGate<Ready>> {
        self.socket.0.receive(1, deadline)?;
        Ok(ExecutionGate {
            socket: self.socket,
            state: PhantomData,
        })
    }
}
impl ExecutionGate<Ready> {
    pub fn start_until(self, deadline: Instant) -> io::Result<()> {
        self.socket.0.send(2, deadline)
    }
}
impl ExecutionEndpoint {
    pub(super) fn into_handle(self) -> OwnedFd {
        self.0.0
    }
    pub(super) fn from_handle(fd: OwnedFd) -> io::Result<Self> {
        if net::sockopt::socket_type(&fd)? != SocketType::SEQPACKET {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "execution gate requires seqpacket",
            ));
        }
        Ok(Self(Socket(fd)))
    }
    pub(super) fn ready_and_wait(self, deadline: Instant) -> io::Result<()> {
        self.0.send(1, deadline)?;
        self.0.receive(2, deadline)
    }
}
impl Socket {
    fn wait(&self, interest: PollFlags, deadline: Instant) -> io::Result<()> {
        loop {
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .filter(|duration| !duration.is_zero())
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::TimedOut, "execution gate deadline expired")
                })?;
            let timeout = Timespec::try_from(remaining).map_err(io::Error::other)?;
            let mut fds = [PollFd::new(&self.0, interest)];
            match poll(&mut fds, Some(&timeout)) {
                Ok(0) => continue,
                Ok(_) if fds[0].revents().contains(interest) => return Ok(()),
                Ok(_) => {
                    return Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "execution gate peer lost",
                    ));
                }
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn send(&self, opcode: u8, deadline: Instant) -> io::Result<()> {
        loop {
            self.wait(PollFlags::OUT, deadline)?;
            match net::send(
                &self.0,
                &[0x82, 1, opcode],
                SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
            ) {
                Ok(3) => return Ok(()),
                Ok(_) => return Err(io::Error::other("short execution gate packet")),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn receive(&self, opcode: u8, deadline: Instant) -> io::Result<()> {
        loop {
            self.wait(PollFlags::IN, deadline)?;
            let mut bytes = [0; 4];
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
            let mut ancillary = RecvAncillaryBuffer::new(&mut space);
            let received = match net::recvmsg(
                &self.0,
                &mut [IoSliceMut::new(&mut bytes)],
                &mut ancillary,
                RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
            ) {
                Ok(received) => received,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            };
            if received.bytes == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "execution gate closed",
                ));
            }
            if received.bytes != 3
                || bytes[..3] != [0x82, 1, opcode]
                || received
                    .flags
                    .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
                || ancillary.drain().next().is_some()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid execution gate packet",
                ));
            }
            return Ok(());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    struct Fixture;
    impl Fixture {
        fn deadline() -> Instant {
            Instant::now() + Duration::from_secs(5)
        }
    }
    #[test]
    fn readiness_requires_explicit_start() {
        let (parent, child) = ExecutionGate::pair().unwrap();
        let worker = std::thread::spawn(move || child.ready_and_wait(Fixture::deadline()));
        let parent = parent.wait_ready_until(Fixture::deadline()).unwrap();
        assert!(!worker.is_finished());
        parent.start_until(Fixture::deadline()).unwrap();
        worker.join().unwrap().unwrap();
    }
    #[test]
    fn peer_loss_expiry_and_wrong_phase_never_authorize() {
        let (parent, child) = ExecutionGate::pair().unwrap();
        assert!(parent.wait_ready_until(Instant::now()).is_err());
        assert!(child.ready_and_wait(Fixture::deadline()).is_err());
        let (parent, child) = ExecutionGate::pair().unwrap();
        child.0.send(2, Fixture::deadline()).unwrap();
        assert_eq!(
            parent
                .wait_ready_until(Fixture::deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
    #[test]
    fn cancellation_after_readiness_does_not_release_command() {
        let (parent, child) = ExecutionGate::pair().unwrap();
        let worker = std::thread::spawn(move || child.ready_and_wait(Fixture::deadline()));
        let parent = parent.wait_ready_until(Fixture::deadline()).unwrap();
        drop(parent);
        assert!(worker.join().unwrap().is_err());
    }
    #[test]
    fn malformed_readiness_and_ancillary_rights_fail_closed() {
        use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage};
        use std::{
            io::{IoSlice, Read},
            os::{fd::AsFd, unix::net::UnixStream},
        };
        for bytes in [&[][..], &[0x82, 2, 1], &[0x82, 1, 1, 0]] {
            let (parent, child) = ExecutionGate::pair().unwrap();
            net::send(&child.0.0, bytes, SendFlags::NOSIGNAL).unwrap();
            assert!(parent.wait_ready_until(Fixture::deadline()).is_err());
        }
        let (parent, child) = ExecutionGate::pair().unwrap();
        let (owner, mut peer) = UnixStream::pair().unwrap();
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        let fds = [owner.as_fd()];
        assert!(ancillary.push(SendAncillaryMessage::ScmRights(&fds)));
        net::sendmsg(
            &child.0.0,
            &[IoSlice::new(&[0x82, 1, 1])],
            &mut ancillary,
            SendFlags::NOSIGNAL,
        )
        .unwrap();
        drop(owner);
        assert_eq!(
            parent
                .wait_ready_until(Fixture::deadline())
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        assert_eq!(peer.read(&mut [0]).unwrap(), 0);
    }
}
