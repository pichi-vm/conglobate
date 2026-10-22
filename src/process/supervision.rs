// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ProcessObservation, identity::IdentitySocket};
use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    net::{AddressFamily, RecvFlags, SocketFlags, SocketType, recv, socketpair},
};
use std::{io, process::Stdio, time::Instant};

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
        self.socket.0.send_current_until(deadline)?;
        let peer = self.socket.0.receive_until(deadline)?;
        Ok(Supervision {
            socket: self.socket,
            peer,
        })
    }
}
impl Supervision<Paired> {
    pub(super) fn duplicate_observation(&self) -> io::Result<ProcessObservation> {
        Ok(ProcessObservation {
            identity: self.peer.identity.try_clone()?,
        })
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
    /// This initial protocol admits no messages after identity exchange.
    pub fn peer_lost(&self) -> io::Result<bool> {
        if self.peer.has_exited()? {
            return Ok(true);
        }
        let mut byte = [0u8];
        match recv(&self.socket.0.0, &mut byte, RecvFlags::DONTWAIT) {
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
