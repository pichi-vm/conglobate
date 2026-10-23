// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::route::RouteRequest;
use netlink_packet_route::{
    RouteNetlinkMessage,
    link::{LinkAttribute, LinkFlags, LinkMessage},
};
use std::{io, time::Instant};

/// Trusted private-namespace setup only; this does not grant external access.
#[derive(Debug)]
pub(crate) struct Loopback;
impl Loopback {
    pub(crate) fn activate(deadline: Instant) -> io::Result<()> {
        let mut request = LinkMessage::default();
        request.attributes.push(LinkAttribute::IfName("lo".into()));
        let replies =
            RouteRequest::new(RouteNetlinkMessage::GetLink(request), 0).execute(deadline)?;
        let [RouteNetlinkMessage::NewLink(link)] = replies.as_slice() else {
            return Err(io::Error::other("missing private loopback"));
        };
        if !link.header.flags.contains(LinkFlags::Loopback) {
            return Err(io::Error::other("invalid private loopback"));
        }
        let mut up = LinkMessage::default();
        up.header.index = link.header.index;
        up.header.flags = LinkFlags::Up;
        up.header.change_mask = LinkFlags::Up;
        RouteRequest::new(RouteNetlinkMessage::NewLink(up), 0).execute(deadline)?;
        Ok(())
    }
}
