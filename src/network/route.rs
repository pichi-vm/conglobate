// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use netlink_packet_core::{
    NLM_F_ACK, NLM_F_DUMP_INTR, NLM_F_REQUEST, NetlinkMessage, NetlinkPayload,
};
use netlink_packet_route::RouteNetlinkMessage;
use rustix::{
    event::{PollFd, PollFlags},
    net::{self, netlink::SocketAddrNetlink},
};
use std::{io, os::fd::OwnedFd, time::Instant};

/// One request per owned socket: an interrupted transaction cannot leave a stale
/// ACK for the next operation. Mutation callers must retain rollback authority
/// even when a send succeeded but its acknowledgement was lost.
#[derive(Debug)]
pub(super) struct RouteRequest {
    message: RouteNetlinkMessage,
    flags: u16,
}
impl RouteRequest {
    pub(super) fn new(message: RouteNetlinkMessage, flags: u16) -> Self {
        Self { message, flags }
    }
    pub(super) fn execute(self, deadline: Instant) -> io::Result<Vec<RouteNetlinkMessage>> {
        let socket = net::socket_with(
            net::AddressFamily::NETLINK,
            net::SocketType::RAW,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            None,
        )?;
        net::bind(&socket, &SocketAddrNetlink::new(0, 0))?;
        let mut packet = NetlinkMessage::from(self.message);
        packet.header.sequence_number = 1;
        packet.header.flags = self.flags | NLM_F_REQUEST | NLM_F_ACK;
        packet.finalize();
        let mut bytes = vec![0; packet.buffer_len()];
        packet.serialize(&mut bytes);
        loop {
            Self::ready(&socket, PollFlags::OUT, deadline)?;
            match net::sendto(
                &socket,
                &bytes,
                net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
                &SocketAddrNetlink::new(0, 0),
            ) {
                Ok(n) if n == bytes.len() => break,
                Ok(_) => return Err(io::Error::other("short route request")),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let mut reply = RouteReply::default();
        let mut buffer = vec![0; 65536];
        loop {
            Self::ready(&socket, PollFlags::IN, deadline)?;
            let (_, length, sender) = match net::recvfrom(
                &socket,
                buffer.as_mut_slice(),
                net::RecvFlags::DONTWAIT | net::RecvFlags::TRUNC,
            ) {
                Ok(result) => result,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            };
            let sender = sender
                .and_then(|address| SocketAddrNetlink::try_from(address).ok())
                .ok_or_else(|| io::Error::other("missing route reply sender"))?;
            if sender.pid() != 0 || sender.groups() != 0 || length > buffer.len() {
                return Err(io::Error::other("invalid route reply sender or truncation"));
            }
            if reply.receive(&buffer[..length])? {
                return Ok(reply.messages);
            }
        }
    }
    pub(super) fn ready(socket: &OwnedFd, flags: PollFlags, deadline: Instant) -> io::Result<()> {
        loop {
            let timeout = crate::deadline::Deadline::at(deadline)
                .remaining("route request deadline expired")?;
            let mut fds = [PollFd::new(socket, flags)];
            match rustix::event::poll(&mut fds, Some(&timeout)) {
                Ok(0) => continue,
                Ok(_) if fds[0].revents().intersects(flags) => return Ok(()),
                Ok(_) => return Err(io::Error::other("route socket closed or failed")),
                Err(rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
    }
}
#[derive(Debug, Default)]
struct RouteReply {
    messages: Vec<RouteNetlinkMessage>,
}
impl RouteReply {
    fn receive(&mut self, bytes: &[u8]) -> io::Result<bool> {
        let mut remaining = bytes;
        let mut acknowledged = false;
        if remaining.is_empty() {
            return Err(io::Error::other("empty route datagram"));
        }
        while !remaining.is_empty() {
            let packet = NetlinkMessage::<RouteNetlinkMessage>::deserialize(remaining)
                .map_err(io::Error::other)?;
            let length = packet.header.length as usize;
            if length < 16
                || length > remaining.len()
                || packet.header.sequence_number != 1
                || packet.header.flags & NLM_F_DUMP_INTR != 0
                || acknowledged
            {
                return Err(io::Error::other("invalid route reply framing or sequence"));
            }
            match packet.payload {
                NetlinkPayload::Error(error) if error.code.is_none() => acknowledged = true,
                NetlinkPayload::Error(error) => return Err(error.to_io()),
                NetlinkPayload::InnerMessage(message) => self.messages.push(message),
                _ => return Err(io::Error::other("unexpected route control message")),
            }
            let aligned = (length + 3) & !3;
            if aligned > remaining.len() {
                if length != remaining.len() {
                    return Err(io::Error::other("truncated route alignment"));
                }
                remaining = &[];
            } else {
                remaining = &remaining[aligned..];
            }
        }
        Ok(acknowledged)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use netlink_packet_core::ErrorMessage;
    #[derive(Debug)]
    struct ReplyFixture;
    impl ReplyFixture {
        fn ack(sequence: u32, error: Option<std::num::NonZeroI32>) -> Vec<u8> {
            let mut failure = ErrorMessage::default();
            failure.code = error;
            let mut packet = NetlinkMessage::<RouteNetlinkMessage>::new(
                Default::default(),
                NetlinkPayload::Error(failure),
            );
            packet.header.sequence_number = sequence;
            packet.finalize();
            let mut bytes = vec![0; packet.buffer_len()];
            packet.serialize(&mut bytes);
            bytes
        }
    }
    #[test]
    fn acknowledgement_is_required_and_rejects_wrong_sequence_truncation_and_nack() {
        let ack = ReplyFixture::ack(1, None);
        assert!(RouteReply::default().receive(&ack).unwrap());
        assert!(
            RouteReply::default()
                .receive(&ReplyFixture::ack(2, None))
                .is_err()
        );
        assert!(
            RouteReply::default()
                .receive(&ack[..ack.len() - 1])
                .is_err()
        );
        assert!(RouteReply::default().receive(&[]).is_err());
        let error = RouteReply::default()
            .receive(&ReplyFixture::ack(1, std::num::NonZeroI32::new(-17)))
            .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(17));
        let mut duplicate = ack.clone();
        duplicate.extend(ack);
        assert!(RouteReply::default().receive(&duplicate).is_err());
    }
}
