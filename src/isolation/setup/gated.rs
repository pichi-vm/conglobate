// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::*;
use crate::isolation::{CommandNamespaces, ExecutionEndpoint};
mod helper;
pub use helper::{GatedRestrictedCommand, HelperExecutionError, RootPreparationError};

#[derive(Debug)]
pub struct GatedCommandSetup {
    setup: CommandSetup,
    gate: ExecutionEndpoint,
}
#[derive(Debug)]
pub struct GatedRootSetup {
    devices: CommandDevices,
    gate: ExecutionEndpoint,
}
impl CommandSetup {
    pub fn with_gate(self, gate: ExecutionEndpoint) -> GatedCommandSetup {
        GatedCommandSetup { setup: self, gate }
    }
}
impl GatedCommandSetup {
    pub fn into_input(self) -> io::Result<Stdio> {
        let mut descriptors = vec![
            self.setup.namespace.into(),
            self.setup.devices.into_handle().into(),
            self.gate.into_handle(),
        ];
        let opcode = if let Some(network) = self.setup.network {
            descriptors.push(network.into());
            5
        } else {
            2
        };
        Packet {
            opcode,
            descriptors,
        }
        .send()
    }
    pub fn from_input() -> io::Result<Self> {
        let socket = io::stdin().as_fd().try_clone_to_owned()?;
        let mut packet = Packet::receive_shapes(&socket, &[(2, 3), (5, 4)])?;
        let network = if packet.opcode == 5 {
            Some(File::from(packet.descriptors.pop().expect("network")))
        } else {
            None
        };
        let gate = ExecutionEndpoint::from_handle(packet.descriptors.pop().expect("gate"))?;
        let devices =
            CommandDevices::from_handle(packet.descriptors.pop().expect("devices").into())?;
        let setup =
            CommandSetup::new(packet.descriptors.pop().expect("namespace").into(), devices)?;
        let setup = match network {
            Some(network) => setup.with_network_handle(network)?,
            None => setup,
        };
        Ok(Self { setup, gate })
    }
    pub fn enter(self) -> io::Result<(CommandNamespaces, GatedRootSetup)> {
        let (scope, devices) = self.setup.enter()?;
        Ok((
            scope,
            GatedRootSetup {
                devices,
                gate: self.gate,
            },
        ))
    }
}
impl GatedRootSetup {
    pub fn into_input(self) -> io::Result<Stdio> {
        Packet {
            opcode: 3,
            descriptors: vec![self.devices.into_handle().into(), self.gate.into_handle()],
        }
        .send()
    }
    pub fn from_input() -> io::Result<Self> {
        let mut packet = Packet::receive(3, 2)?;
        let gate = ExecutionEndpoint::from_handle(packet.descriptors.pop().expect("gate"))?;
        let devices =
            CommandDevices::from_handle(packet.descriptors.pop().expect("devices").into())?;
        Ok(Self { devices, gate })
    }
    pub fn into_parts(self) -> (CommandDevices, ExecutionEndpoint) {
        (self.devices, self.gate)
    }
}
#[derive(Debug)]
struct Packet {
    opcode: u8,
    descriptors: Vec<OwnedFd>,
}
impl Packet {
    fn send(self) -> io::Result<Stdio> {
        let (sender, receiver) = UnixDatagram::pair()?;
        let descriptors: Vec<_> = self.descriptors.iter().map(AsFd::as_fd).collect();
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(io::Error::other("gated setup capacity"));
        }
        if sendmsg(
            &sender,
            &[IoSlice::new(&[0x82, 1, self.opcode])],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )? != 3
        {
            return Err(io::Error::other("short gated setup packet"));
        }
        Ok(OwnedFd::from(receiver).into())
    }
    fn receive(opcode: u8, count: usize) -> io::Result<Self> {
        let socket = io::stdin().as_fd().try_clone_to_owned()?;
        Self::receive_from(&socket, opcode, count)
    }
    fn receive_from(socket: &impl AsFd, opcode: u8, count: usize) -> io::Result<Self> {
        Self::receive_shapes(socket, &[(opcode, count)])
    }
    fn receive_shapes(socket: &impl AsFd, shapes: &[(u8, usize)]) -> io::Result<Self> {
        let mut bytes = [0; 4];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(5))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = recvmsg(
            socket,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        )?;
        let mut descriptors = Vec::new();
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
                _ => return Err(io::Error::other("unexpected gated setup control")),
            }
        }
        if received.bytes != 3
            || bytes[..2] != [0x82, 1]
            || !shapes.contains(&(bytes[2], descriptors.len()))
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(io::Error::other("invalid gated setup packet"));
        }
        Ok(Self {
            opcode: bytes[2],
            descriptors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, os::unix::net::UnixStream, time::Duration};
    struct Fixture;
    impl Fixture {
        fn reject(bytes: &[u8], count: usize) {
            let (sender, receiver) = UnixDatagram::pair().unwrap();
            let (owners, peers): (Vec<_>, Vec<_>) =
                (0..count).map(|_| UnixStream::pair().unwrap()).unzip();
            let fds: Vec<_> = owners.iter().map(AsFd::as_fd).collect();
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(5))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if count != 0 {
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(&fds)));
            }
            sendmsg(
                &sender,
                &[IoSlice::new(bytes)],
                &mut ancillary,
                SendFlags::NOSIGNAL,
            )
            .unwrap();
            drop(owners);
            assert!(Packet::receive_from(&receiver, 2, 3).is_err());
            for mut peer in peers {
                peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                assert_eq!(peer.read(&mut [0]).unwrap(), 0);
            }
        }
    }
    #[test]
    fn gated_setup_rejects_wrong_shapes_and_releases_all_rights() {
        for count in [0, 1, 2, 4, 5] {
            Fixture::reject(&[0x82, 1, 2], count);
        }
        for bytes in [
            &[][..],
            &[0x82, 1],
            &[0x82, 2, 2],
            &[0x82, 1, 3],
            &[0x82, 1, 2, 0],
        ] {
            Fixture::reject(bytes, 3);
        }
    }
}
