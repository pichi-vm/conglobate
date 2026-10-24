// SPDX-License-Identifier: Apache-2.0
use rustix::{
    fd::{AsFd, BorrowedFd, OwnedFd},
    net::{
        self, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendAncillaryBuffer, SendAncillaryMessage, SendFlags,
    },
};
use std::{
    io::{self, IoSlice, IoSliceMut},
    mem::MaybeUninit,
};

/// One nonblocking message and its owned descriptors. Protocols retain their
/// framing, count/type validation and wait policy. No retries or waits occur here.
#[derive(Debug)]
pub(crate) struct DescriptorPacket {
    pub(crate) bytes: usize,
    pub(crate) descriptors: Vec<OwnedFd>,
}
impl DescriptorPacket {
    pub(crate) fn receive_now(socket: &impl AsFd, bytes: &mut [u8]) -> io::Result<Self> {
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let received = net::recvmsg(
            socket,
            &mut [IoSliceMut::new(bytes)],
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        )?;
        let mut descriptors = Vec::new();
        for message in ancillary.drain() {
            match message {
                RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "unexpected ancillary message",
                    ));
                }
            }
        }
        if received
            .flags
            .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated descriptor packet",
            ));
        }
        Ok(Self {
            bytes: received.bytes,
            descriptors,
        })
    }
    pub(crate) fn send_now(
        socket: &impl AsFd,
        bytes: &[u8],
        descriptors: &[BorrowedFd<'_>],
    ) -> io::Result<()> {
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !descriptors.is_empty() && !ancillary.push(SendAncillaryMessage::ScmRights(descriptors))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "descriptor packet capacity",
            ));
        }
        if net::sendmsg(
            socket,
            &[IoSlice::new(bytes)],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )? != bytes.len()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "short descriptor packet",
            ));
        }
        Ok(())
    }
}
