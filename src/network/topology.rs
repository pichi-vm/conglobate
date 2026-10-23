// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{InterfaceName, NetworkAddress, route::RouteRequest};
use netlink_packet_route::{
    AddressFamily, RouteNetlinkMessage,
    address::{AddressAttribute, AddressMessage},
    link::{LinkAttribute, LinkMessage},
    route::{RouteAddress, RouteAttribute, RouteMessage},
};
use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    time::Instant,
};

/// Read-only observation of the supervisor network. Allocation avoids assigned
/// prefixes and non-default routes in every table. Exclusive link creation is
/// still required: this observation does not reserve names or addresses.
#[derive(Debug)]
pub struct NetworkTopology {
    prefixes: Vec<Prefix>,
    names: Vec<String>,
}
#[derive(Debug, Clone, Copy)]
struct Prefix {
    first: u128,
    last: u128,
    ipv4: bool,
}
impl Prefix {
    fn new(address: IpAddr, bits: u8) -> io::Result<Self> {
        let (address, width, ipv4) = match address {
            IpAddr::V4(a) => (u32::from(a) as u128, 32, true),
            IpAddr::V6(a) => (u128::from(a), 128, false),
        };
        if bits > width {
            return Err(io::Error::other("invalid network prefix length"));
        }
        let host_mask = u128::MAX
            .checked_shr(128 - u32::from(width - bits))
            .unwrap_or(0);
        Ok(Self {
            first: address & !host_mask,
            last: address | host_mask,
            ipv4,
        })
    }
    fn available(self, occupied: &[Self]) -> Option<u128> {
        let mut first = self.first;
        loop {
            let last = first.checked_add(3)?;
            if last > self.last {
                return None;
            }
            match occupied
                .iter()
                .filter(|p| p.ipv4 == self.ipv4 && p.first <= last && p.last >= first)
                .map(|p| p.last)
                .max()
            {
                None => return Some(first),
                Some(end) => first = end.checked_add(1)?.checked_add(3)? & !3,
            }
        }
    }
}
/// Private /30 and /126 links selected from a complete topology observation.
/// Host and peer addresses are separate from environment routes and DNS.
#[derive(Debug)]
pub struct LinkAllocation {
    host: InterfaceName,
    peer: InterfaceName,
    ipv4: u32,
    ipv6: u128,
}
impl LinkAllocation {
    pub fn host_name(&self) -> &InterfaceName {
        &self.host
    }
    pub fn peer_name(&self) -> &InterfaceName {
        &self.peer
    }
    pub fn addresses(&self) -> io::Result<(Vec<NetworkAddress>, Vec<NetworkAddress>)> {
        let host4 = IpAddr::V4(Ipv4Addr::from(self.ipv4 + 1));
        let peer4 = IpAddr::V4(Ipv4Addr::from(self.ipv4 + 2));
        let host6 = IpAddr::V6(Ipv6Addr::from(self.ipv6 + 1));
        let peer6 = IpAddr::V6(Ipv6Addr::from(self.ipv6 + 2));
        Ok((
            vec![
                NetworkAddress::new(host4, 30, None)?,
                NetworkAddress::new(host6, 126, None)?,
            ],
            vec![
                NetworkAddress::new(peer4, 30, Some(host4))?,
                NetworkAddress::new(peer6, 126, Some(host6))?,
            ],
        ))
    }
}
impl NetworkTopology {
    pub fn capture(deadline: Instant) -> io::Result<Self> {
        let mut topology = Self {
            prefixes: Vec::new(),
            names: Vec::new(),
        };
        for message in RouteRequest::dump(RouteNetlinkMessage::GetLink(LinkMessage::default()))
            .execute(deadline)?
        {
            let RouteNetlinkMessage::NewLink(link) = message else {
                return Err(io::Error::other("unexpected link dump entry"));
            };
            topology
                .names
                .extend(link.attributes.into_iter().filter_map(|a| match a {
                    LinkAttribute::IfName(name) => Some(name),
                    _ => None,
                }));
        }
        for family in [AddressFamily::Inet, AddressFamily::Inet6] {
            let mut request = AddressMessage::default();
            request.header.family = family;
            for message in
                RouteRequest::dump(RouteNetlinkMessage::GetAddress(request)).execute(deadline)?
            {
                let RouteNetlinkMessage::NewAddress(address) = message else {
                    return Err(io::Error::other("unexpected address dump entry"));
                };
                for attribute in address.attributes {
                    if let AddressAttribute::Address(ip) | AddressAttribute::Local(ip) = attribute {
                        topology
                            .prefixes
                            .push(Prefix::new(ip, address.header.prefix_len)?);
                    }
                }
            }
            let mut request = RouteMessage::default();
            request.header.address_family = family;
            for message in
                RouteRequest::dump(RouteNetlinkMessage::GetRoute(request)).execute(deadline)?
            {
                let RouteNetlinkMessage::NewRoute(route) = message else {
                    return Err(io::Error::other("unexpected route dump entry"));
                };
                // Default routes provide upstream connectivity; other prefixes
                // must remain reachable through their existing routes.
                if route.header.destination_prefix_length == 0 {
                    continue;
                }
                let destination = route
                    .attributes
                    .into_iter()
                    .find_map(|a| match a {
                        RouteAttribute::Destination(RouteAddress::Inet(ip)) => Some(IpAddr::V4(ip)),
                        RouteAttribute::Destination(RouteAddress::Inet6(ip)) => {
                            Some(IpAddr::V6(ip))
                        }
                        _ => None,
                    })
                    .ok_or_else(|| io::Error::other("route prefix missing destination"))?;
                topology.prefixes.push(Prefix::new(
                    destination,
                    route.header.destination_prefix_length,
                )?);
            }
        }
        Ok(topology)
    }
    pub fn allocate(&self) -> io::Result<LinkAllocation> {
        let names = (0..65536)
            .find_map(|n| {
                let host = format!("cgb{n}h");
                let peer = format!("cgb{n}p");
                (!self.names.contains(&host) && !self.names.contains(&peer)).then_some((host, peer))
            })
            .ok_or_else(|| io::Error::other("no unused command interface names"))?;
        let ipv4 = [
            (Ipv4Addr::new(10, 0, 0, 0), 8),
            (Ipv4Addr::new(172, 16, 0, 0), 12),
            (Ipv4Addr::new(192, 168, 0, 0), 16),
        ]
        .into_iter()
        .find_map(|(ip, bits)| {
            Prefix::new(ip.into(), bits)
                .expect("fixed private prefix")
                .available(&self.prefixes)
        })
        .ok_or_else(|| io::Error::other("no unused private IPv4 link prefix"))?
            as u32;
        let ipv6 = Prefix::new(Ipv6Addr::from(0xfd_u128 << 120).into(), 8)?
            .available(&self.prefixes)
            .ok_or_else(|| io::Error::other("no unused private IPv6 link prefix"))?;
        Ok(LinkAllocation {
            host: InterfaceName::try_from(names.0.as_str())?,
            peer: InterfaceName::try_from(names.1.as_str())?,
            ipv4,
            ipv6,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_skips_routes_addresses_and_both_link_names() {
        let topology = NetworkTopology {
            names: vec!["cgb0p".into(), "cgb1h".into()],
            prefixes: vec![
                Prefix::new("10.0.0.0".parse().unwrap(), 8).unwrap(),
                Prefix::new("172.16.0.0".parse().unwrap(), 12).unwrap(),
                Prefix::new("192.168.0.2".parse().unwrap(), 32).unwrap(),
                Prefix::new("fd00::".parse().unwrap(), 64).unwrap(),
            ],
        };
        let allocation = topology.allocate().unwrap();
        assert_eq!(allocation.host_name().as_ref(), "cgb2h");
        assert_eq!(allocation.peer_name().as_ref(), "cgb2p");
        assert_eq!(allocation.ipv4, u32::from(Ipv4Addr::new(192, 168, 0, 4)));
        assert_eq!(allocation.ipv6, (0xfd_u128 << 120) + (1_u128 << 64));
        let (host, peer) = allocation.addresses().unwrap();
        assert_eq!((host.len(), peer.len()), (2, 2));
    }
    #[test]
    fn exhaustion_does_not_reuse_an_occupied_prefix() {
        let topology = NetworkTopology {
            names: Vec::new(),
            prefixes: vec![Prefix::new("0.0.0.0".parse().unwrap(), 0).unwrap()],
        };
        assert!(topology.allocate().is_err());
        let topology = NetworkTopology {
            names: Vec::new(),
            prefixes: vec![Prefix::new("fd00::".parse().unwrap(), 8).unwrap()],
        };
        assert!(topology.allocate().is_err());
    }
    #[test]
    fn prefix_arithmetic_handles_host_routes_and_full_width() {
        let host = Prefix::new(
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff".parse().unwrap(),
            128,
        )
        .unwrap();
        assert_eq!((host.first, host.last), (u128::MAX, u128::MAX));
        let all = Prefix::new("::".parse().unwrap(), 0).unwrap();
        assert_eq!((all.first, all.last), (0, u128::MAX));
        assert!(all.available(&[all]).is_none());
        assert!(Prefix::new("1.2.3.4".parse().unwrap(), 33).is_err());
    }
    #[test]
    fn captures_current_network_without_mutation() {
        let topology =
            NetworkTopology::capture(Instant::now() + std::time::Duration::from_secs(5)).unwrap();
        assert!(topology.names.iter().any(|name| name == "lo"));
    }
}
