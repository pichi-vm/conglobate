// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CommandDevices, CommandNamespaces};
use rustix::{
    fd::{AsFd, OwnedFd},
    net::{
        RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
        SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
    },
};
use std::{
    fs::{File, Metadata},
    io::{self, IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::unix::net::UnixDatagram,
    process::Stdio,
};

/// One-shot trusted setup input: the selected namespace and original private
/// device mount. Neither descriptor is exposed to image commands. This is not
/// an authenticated input format for an untrusted sender or command stream.
#[derive(Debug)]
pub struct CommandSetup {
    namespace: File,
    devices: CommandDevices,
}
impl CommandSetup {
    pub fn new(namespace: File, devices: CommandDevices) -> io::Result<Self> {
        if rustix::fs::fstatfs(&namespace)?.f_type as u64 != 0x6e73_6673 {
            return Err(io::Error::other("setup namespace requires nsfs"));
        }
        Ok(Self { namespace, devices })
    }
    pub fn namespace_metadata(&self) -> io::Result<Metadata> {
        self.namespace.metadata()
    }

    pub fn into_input(self) -> io::Result<Stdio> {
        let (sender, receiver) = UnixDatagram::pair()?;
        let device = self.devices.into_handle();
        let descriptors = [self.namespace.as_fd(), device.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(io::Error::other("setup transfer ancillary capacity"));
        }
        if sendmsg(
            &sender,
            &[IoSlice::new(&[0x82, 1, 1])],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )? != 3
        {
            return Err(io::Error::other("incomplete setup transfer"));
        }
        Ok(Stdio::from(OwnedFd::from(receiver)))
    }

    pub fn from_input() -> io::Result<Self> {
        let socket = io::stdin().as_fd().try_clone_to_owned()?;
        Self::receive(&socket)
    }
    fn receive(socket: &impl AsFd) -> io::Result<Self> {
        let mut bytes = [0; 4];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = recvmsg(
            socket,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT,
        )?;
        let mut descriptors = Vec::new();
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
                _ => return Err(io::Error::other("unexpected setup ancillary message")),
            }
        }
        if received.bytes != 3
            || bytes[..3] != [0x82, 1, 1]
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || descriptors.len() != 2
        {
            return Err(io::Error::other("invalid setup transfer packet"));
        }
        let devices = CommandDevices::from_handle(
            descriptors.pop().expect("checked device descriptor").into(),
        )?;
        Self::new(
            descriptors
                .pop()
                .expect("checked namespace descriptor")
                .into(),
            devices,
        )
    }

    /// Enter only in a fresh trusted setup process. Partial namespace transition
    /// is exit-only. The remaining device owner must be passed to root setup;
    /// this does not authorize image execution or certify complete isolation.
    pub fn enter(self) -> io::Result<(CommandNamespaces, CommandDevices)> {
        Ok((CommandNamespaces::enter(self.namespace)?, self.devices))
    }
}

