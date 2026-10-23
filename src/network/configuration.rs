// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{InterfaceName, route::RouteRequest};
use netlink_packet_core::{NLM_F_CREATE, NLM_F_EXCL};
use netlink_packet_route::{
    AddressFamily, RouteNetlinkMessage,
    address::{AddressAttribute, AddressHeaderFlags, AddressMessage},
    link::{LinkAttribute, LinkFlags, LinkMessage},
    route::{RouteAddress, RouteAttribute, RouteMessage, RouteProtocol, RouteScope, RouteType},
};
use std::{io, net::IpAddr, time::Instant};

/// One address on a private link, with an optional same-family default gateway.
#[derive(Debug, Clone)]
pub struct NetworkAddress {
    address: IpAddr,
    prefix: u8,
    gateway: Option<IpAddr>,
}
impl NetworkAddress {
    pub fn new(address: IpAddr, prefix: u8, gateway: Option<IpAddr>) -> io::Result<Self> {
        if address.is_unspecified()
            || address.is_multicast()
            || address.is_loopback()
            || prefix == 0
            || prefix > if address.is_ipv4() { 32 } else { 128 }
            || gateway.is_some_and(|gateway| {
                gateway.is_ipv4() != address.is_ipv4()
                    || gateway == address
                    || gateway.is_unspecified()
                    || gateway.is_multicast()
                    || gateway.is_loopback()
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid private link address/prefix/gateway",
            ));
        }
        Ok(Self {
            address,
            prefix,
            gateway,
        })
    }
    pub(super) fn has_gateway(&self) -> bool {
        self.gateway.is_some()
    }
    fn family(&self) -> AddressFamily {
        if self.address.is_ipv4() {
            AddressFamily::Inet
        } else {
            AddressFamily::Inet6
        }
    }
    fn apply(&self, index: u32, deadline: Instant) -> io::Result<()> {
        let mut address = AddressMessage::default();
        address.header.family = self.family();
        address.header.prefix_len = self.prefix;
        address.header.index = index;
        if self.address.is_ipv6() {
            address.header.flags = AddressHeaderFlags::Nodad;
        }
        address.attributes = vec![
            AddressAttribute::Address(self.address),
            AddressAttribute::Local(self.address),
        ];
        RouteRequest::new(
            RouteNetlinkMessage::NewAddress(address),
            NLM_F_CREATE | NLM_F_EXCL,
        )
        .execute(deadline)?;
        Ok(())
    }
    fn route(&self, index: u32, deadline: Instant) -> io::Result<()> {
        if let Some(gateway) = self.gateway {
            let mut route = RouteMessage::default();
            route.header.address_family = self.family();
            route.header.table = 254;
            route.header.protocol = RouteProtocol::Static;
            route.header.scope = RouteScope::Universe;
            route.header.kind = RouteType::Unicast;
            let gateway = match gateway {
                IpAddr::V4(a) => RouteAddress::Inet(a),
                IpAddr::V6(a) => RouteAddress::Inet6(a),
            };
            route.attributes = vec![RouteAttribute::Oif(index), RouteAttribute::Gateway(gateway)];
            RouteRequest::new(
                RouteNetlinkMessage::NewRoute(route),
                NLM_F_CREATE | NLM_F_EXCL,
            )
            .execute(deadline)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LinkConfiguration {
    name: InterfaceName,
    mac: Vec<u8>,
    addresses: Vec<NetworkAddress>,
}
impl LinkConfiguration {
    pub(super) fn new(
        name: InterfaceName,
        mac: Vec<u8>,
        addresses: Vec<NetworkAddress>,
    ) -> io::Result<Self> {
        if mac.len() != 6
            || addresses.is_empty()
            || addresses.len() > 2
            || (addresses.len() == 2
                && addresses[0].address.is_ipv4() == addresses[1].address.is_ipv4())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "expected one private address per family",
            ));
        }
        Ok(Self {
            name,
            mac,
            addresses,
        })
    }
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut e = minicbor::Encoder::new(Vec::new());
        e.array(4)
            .unwrap()
            .u8(1)
            .unwrap()
            .str(self.name.as_ref())
            .unwrap()
            .bytes(&self.mac)
            .unwrap()
            .array(self.addresses.len() as u64)
            .unwrap();
        for address in &self.addresses {
            e.array(3)
                .unwrap()
                .str(&address.address.to_string())
                .unwrap()
                .u8(address.prefix)
                .unwrap();
            match address.gateway {
                Some(gateway) => {
                    e.str(&gateway.to_string()).unwrap();
                }
                None => {
                    e.null().unwrap();
                }
            }
        }
        e.into_writer()
    }
    pub(crate) fn decode(bytes: &[u8]) -> io::Result<Self> {
        let result = (|| -> Result<Self, Box<dyn std::error::Error>> {
            let mut d = minicbor::Decoder::new(bytes);
            if d.array()? != Some(4) || d.u8()? != 1 {
                return Err("invalid link configuration version/shape".into());
            }
            let name = InterfaceName::try_from(d.str()?)?;
            let mac = d.bytes()?.to_vec();
            let count = d
                .array()?
                .filter(|n| (1..=2).contains(n))
                .ok_or("invalid address count")?;
            let mut addresses = Vec::new();
            for _ in 0..count {
                if d.array()? != Some(3) {
                    return Err("invalid address shape".into());
                }
                let address = d.str()?.parse()?;
                let prefix = d.u8()?;
                let gateway = if d.datatype()? == minicbor::data::Type::Null {
                    d.null()?;
                    None
                } else {
                    Some(d.str()?.parse()?)
                };
                addresses.push(NetworkAddress::new(address, prefix, gateway)?);
            }
            let value = Self::new(name, mac, addresses)?;
            if d.position() != bytes.len() || value.encode() != bytes {
                return Err("noncanonical link configuration".into());
            }
            Ok(value)
        })();
        result.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
    }
    pub(super) fn apply(&self, deadline: Instant) -> io::Result<()> {
        let mut request = LinkMessage::default();
        request
            .attributes
            .push(LinkAttribute::IfName(self.name.as_ref().into()));
        let replies =
            RouteRequest::new(RouteNetlinkMessage::GetLink(request), 0).execute(deadline)?;
        let [RouteNetlinkMessage::NewLink(link)] = replies.as_slice() else {
            return Err(io::Error::other("ambiguous private link lookup"));
        };
        if !link
            .attributes
            .contains(&LinkAttribute::Address(self.mac.clone()))
        {
            return Err(io::Error::other("private link identity changed"));
        }
        for address in &self.addresses {
            address.apply(link.header.index, deadline)?;
        }
        let mut up = LinkMessage::default();
        up.header.index = link.header.index;
        up.header.flags = LinkFlags::Up;
        up.header.change_mask = LinkFlags::Up;
        RouteRequest::new(RouteNetlinkMessage::NewLink(up), 0).execute(deadline)?;
        for address in &self.addresses {
            address.route(link.header.index, deadline)?;
        }
        Ok(())
    }
    /// Fresh trusted helper only; stdin is the owned namespace descriptor.
    pub(crate) fn serve(self, deadline: Instant) -> io::Result<()> {
        use std::os::fd::AsFd;
        if std::fs::read_dir("/proc/self/task")?
            .collect::<io::Result<Vec<_>>>()?
            .len()
            != 1
        {
            return Err(io::Error::other("network setup requires one thread"));
        }
        let namespace = io::stdin().as_fd().try_clone_to_owned()?;
        nix::sched::setns(&namespace, nix::sched::CloneFlags::CLONE_NEWNET)?;
        drop(namespace);
        self.apply(deadline)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_roundtrip_is_canonical_and_validated() {
        let address = NetworkAddress::new(
            "10.44.0.2".parse().unwrap(),
            30,
            Some("10.44.0.1".parse().unwrap()),
        )
        .unwrap();
        let config = LinkConfiguration::new(
            InterfaceName::try_from("cg-peer").unwrap(),
            vec![2, 1, 2, 3, 4, 5],
            vec![address.clone()],
        )
        .unwrap();
        let bytes = config.encode();
        assert_eq!(LinkConfiguration::decode(&bytes).unwrap().encode(), bytes);
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(LinkConfiguration::decode(&trailing).is_err());
        assert!(LinkConfiguration::decode(&bytes[..bytes.len() - 1]).is_err());
        assert!(
            LinkConfiguration::new(
                config.name.clone(),
                config.mac.clone(),
                vec![address.clone(), address]
            )
            .is_err()
        );
        assert!(NetworkAddress::new("10.44.0.2".parse().unwrap(), 33, None).is_err());
        assert!(
            NetworkAddress::new(
                "10.44.0.2".parse().unwrap(),
                30,
                Some("fd42::1".parse().unwrap())
            )
            .is_err()
        );
        assert!(NetworkAddress::new("::".parse().unwrap(), 64, None).is_err());
    }
}
