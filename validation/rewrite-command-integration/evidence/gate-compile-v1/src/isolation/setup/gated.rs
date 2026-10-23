// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::*;
use crate::isolation::{CommandNamespaces, ExecutionEndpoint};

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
        Packet {
            opcode: 2,
            descriptors: vec![
                self.setup.namespace.into(),
                self.setup.devices.into_handle().into(),
                self.gate.into_handle(),
            ],
        }
        .send()
    }
    pub fn from_input() -> io::Result<Self> {
        let mut packet = Packet::receive(2, 3)?;
        let gate = ExecutionEndpoint::from_handle(packet.descriptors.pop().expect("gate"))?;
        let devices =
            CommandDevices::from_handle(packet.descriptors.pop().expect("devices").into())?;
        Ok(Self {
            setup: CommandSetup::new(packet.descriptors.pop().expect("namespace").into(), devices)?,
            gate,
        })
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
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
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
        let mut bytes = [0; 4];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = recvmsg(
            &socket,
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
            || bytes[..3] != [0x82, 1, opcode]
            || descriptors.len() != count
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(io::Error::other("invalid gated setup packet"));
        }
        Ok(Self {
            opcode,
            descriptors,
        })
    }
}
