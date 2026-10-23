// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::route::RouteRequest;
use netlink_packet_core::{NLM_F_ACK, NLM_F_REQUEST, NetlinkMessage, NetlinkPayload};
use netlink_packet_netfilter::{
    NetfilterHeader, NetfilterMessage, NetfilterProtoFamily, NetfilterSubsystem,
    nftables::NfTablesMessage, none::ControlMessage,
};
use rustix::{
    event::PollFlags,
    net::{self, netlink::SocketAddrNetlink},
};
use std::{collections::BTreeSet, io, os::fd::OwnedFd, time::Instant};

/// A dedicated, nonblocking socket owns this build's nftables table. Requests
/// use monotonically increasing sequences; no multicast subscription is made.
#[derive(Debug)]
pub(super) struct NetfilterSocket {
    socket: OwnedFd,
    sequence: u32,
}
impl NetfilterSocket {
    pub(super) fn new() -> io::Result<Self> {
        let socket = net::socket_with(
            net::AddressFamily::NETLINK,
            net::SocketType::RAW,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            Some(net::netlink::NETFILTER),
        )?;
        net::bind(&socket, &SocketAddrNetlink::new(0, 0))?;
        Ok(Self {
            socket,
            sequence: 0,
        })
    }
    fn packet(&mut self, message: NetfilterMessage, flags: u16) -> io::Result<(u32, Vec<u8>)> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| io::Error::other("netfilter sequence exhausted"))?;
        let mut packet = NetlinkMessage::from(message);
        packet.header.sequence_number = self.sequence;
        packet.header.flags = flags | NLM_F_REQUEST;
        packet.finalize();
        let mut bytes = vec![0; packet.buffer_len()];
        packet.serialize(&mut bytes);
        Ok((self.sequence, bytes))
    }
    pub(super) fn transact(
        &mut self,
        requests: Vec<(NfTablesMessage, u16)>,
        deadline: Instant,
    ) -> io::Result<()> {
        if requests.is_empty() {
            return Err(io::Error::other("empty nftables transaction"));
        }
        let begin = NetfilterMessage::new(
            NetfilterHeader::new(
                NetfilterProtoFamily::Unspec,
                0,
                u8::from(NetfilterSubsystem::NfTables) as u16,
            ),
            ControlMessage::BatchBegin,
        );
        let (first, mut bytes) = self.packet(begin, 0)?;
        let mut expected = BTreeSet::new();
        for (request, flags) in requests {
            let (sequence, packet) = self.packet(
                NetfilterMessage::new(
                    NetfilterHeader::new(NetfilterProtoFamily::Inet, 0, 0),
                    request,
                ),
                flags | NLM_F_ACK,
            )?;
            expected.insert(sequence);
            bytes.extend(packet);
        }
        let end = NetfilterMessage::new(
            NetfilterHeader::new(
                NetfilterProtoFamily::Unspec,
                0,
                u8::from(NetfilterSubsystem::NfTables) as u16,
            ),
            ControlMessage::BatchEnd,
        );
        let (_, packet) = self.packet(end, 0)?;
        bytes.extend(packet);
        if bytes.len() > 65536 {
            return Err(io::Error::other(
                "nftables transaction exceeds bounded datagram",
            ));
        }
        loop {
            RouteRequest::ready(&self.socket, PollFlags::OUT, deadline)?;
            match net::sendto(
                &self.socket,
                &bytes,
                net::SendFlags::DONTWAIT | net::SendFlags::NOSIGNAL,
                &SocketAddrNetlink::new(0, 0),
            ) {
                Ok(n) if n == bytes.len() => break,
                Ok(_) => return Err(io::Error::other("short nftables transaction")),
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        let mut reply = BatchReply {
            first,
            last: self.sequence,
            expected,
        };
        let mut buffer = vec![0; 65536];
        while !reply.expected.is_empty() {
            RouteRequest::ready(&self.socket, PollFlags::IN, deadline)?;
            let (_, length, sender) = match net::recvfrom(
                &self.socket,
                buffer.as_mut_slice(),
                net::RecvFlags::DONTWAIT | net::RecvFlags::TRUNC,
            ) {
                Ok(reply) => reply,
                Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
                Err(error) => return Err(error.into()),
            };
            let sender = sender
                .and_then(|s| SocketAddrNetlink::try_from(s).ok())
                .ok_or_else(|| io::Error::other("missing netfilter sender"))?;
            if sender.pid() != 0 || sender.groups() != 0 || length > buffer.len() {
                return Err(io::Error::other("invalid netfilter sender or truncation"));
            }
            reply.receive(&buffer[..length])?;
        }
        Ok(())
    }
}
#[derive(Debug)]
struct BatchReply {
    first: u32,
    last: u32,
    expected: BTreeSet<u32>,
}
impl BatchReply {
    fn receive(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.is_empty() {
            return Err(io::Error::other("empty netfilter reply"));
        }
        let mut remaining = bytes;
        while !remaining.is_empty() {
            let packet = NetlinkMessage::<NetfilterMessage>::deserialize(remaining)
                .map_err(io::Error::other)?;
            let length = packet.header.length as usize;
            let sequence = packet.header.sequence_number;
            if length < 16 || length > remaining.len() || sequence > self.last || sequence == 0 {
                return Err(io::Error::other("invalid netfilter reply framing/sequence"));
            }
            // A failed earlier transaction may have unread kernel ACKs. They
            // cannot acknowledge this transaction, and consume the same deadline.
            if sequence >= self.first {
                match packet.payload {
                    NetlinkPayload::Error(error) if error.code.is_some() => {
                        return Err(error.to_io());
                    }
                    NetlinkPayload::Error(_) if self.expected.remove(&sequence) => (),
                    _ => {
                        return Err(io::Error::other(
                            "unexpected or duplicate netfilter acknowledgement",
                        ));
                    }
                }
            }
            let aligned = (length + 3) & !3;
            if aligned > remaining.len() {
                if length != remaining.len() {
                    return Err(io::Error::other("truncated netfilter alignment"));
                }
                remaining = &[];
            } else {
                remaining = &remaining[aligned..];
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture;
    impl Fixture {
        fn ack(sequence: u32, code: Option<std::num::NonZeroI32>) -> Vec<u8> {
            let mut error = netlink_packet_core::ErrorMessage::default();
            error.code = code;
            let mut packet = NetlinkMessage::<NetfilterMessage>::new(
                Default::default(),
                NetlinkPayload::Error(error),
            );
            packet.header.sequence_number = sequence;
            packet.finalize();
            let mut bytes = vec![0; packet.buffer_len()];
            packet.serialize(&mut bytes);
            bytes
        }
        fn reply() -> BatchReply {
            BatchReply {
                first: 4,
                last: 7,
                expected: [5, 6].into(),
            }
        }
    }
    #[test]
    fn batches_require_each_ack_and_reject_duplicates_and_wrong_sequences() {
        let mut reply = Fixture::reply();
        reply.receive(&Fixture::ack(2, None)).unwrap();
        assert_eq!(reply.expected.len(), 2);
        reply.receive(&Fixture::ack(5, None)).unwrap();
        assert_eq!(reply.expected.len(), 1);
        assert!(reply.receive(&Fixture::ack(5, None)).is_err());
        reply.receive(&Fixture::ack(6, None)).unwrap();
        assert!(reply.expected.is_empty());
        assert!(Fixture::reply().receive(&Fixture::ack(8, None)).is_err());
        assert!(Fixture::reply().receive(&Fixture::ack(4, None)).is_err());
        assert!(Fixture::reply().receive(&[]).is_err());
        let ack = Fixture::ack(5, None);
        assert!(Fixture::reply().receive(&ack[..ack.len() - 1]).is_err());
        assert_eq!(
            Fixture::reply()
                .receive(&Fixture::ack(6, std::num::NonZeroI32::new(-17)))
                .unwrap_err()
                .raw_os_error(),
            Some(17)
        );
    }
}
