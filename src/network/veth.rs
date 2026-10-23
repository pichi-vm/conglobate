// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{NetworkNamespace, route::RouteRequest};
use crate::storage::FailureDiagnostics;
use netlink_packet_core::{NLM_F_CREATE, NLM_F_EXCL};
use netlink_packet_route::{
    RouteNetlinkMessage,
    link::{InfoData, InfoKind, InfoVeth, LinkAttribute, LinkInfo, LinkMessage},
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io,
    mem::ManuallyDrop,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceName(String);
impl TryFrom<&str> for InterfaceName {
    type Error = io::Error;
    fn try_from(name: &str) -> io::Result<Self> {
        if name.is_empty()
            || name.len() >= 16
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid private interface name",
            ));
        }
        Ok(Self(name.into()))
    }
}
impl AsRef<str> for InterfaceName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Owns an exclusively created pair, initially DOWN and without addresses.
/// The control marker deliberately survives abandonment, preventing its trusted
/// parent's empty-directory cleanup from claiming that the links were removed.
/// Explicit close checks the pinned network namespace and private ownership tag.
/// The dedicated VM is responsible for disposal after unrecoverable failure.
#[derive(Debug)]
pub struct VethPair {
    host: InterfaceName,
    peer: InterfaceName,
    host_address: Vec<u8>,
    peer_address: Vec<u8>,
    namespace: File,
    marker: ManuallyDrop<tempfile::TempDir>,
    _parent: File,
}
#[derive(Debug, thiserror::Error)]
#[error("veth: {diagnostics}")]
pub struct VethFailure {
    diagnostics: FailureDiagnostics,
    pair: Option<Box<VethPair>>,
    deadline: Instant,
}
impl VethFailure {
    fn before(error: io::Error) -> Self {
        Self {
            diagnostics: error.into(),
            pair: None,
            deadline: Instant::now(),
        }
    }
    fn retained(error: io::Error, pair: Box<VethPair>, deadline: Instant) -> Self {
        Self {
            diagnostics: error.into(),
            pair: Some(pair),
            deadline,
        }
    }
    /// Retry within the original cleanup cutoff. A rejected creation cannot
    /// become a successful acquisition through cleanup.
    pub fn cleanup(self) -> Result<(), Self> {
        let Self {
            diagnostics,
            pair,
            deadline,
        } = self;
        match pair {
            None => Ok(()),
            Some(pair) => match pair.release(deadline) {
                Ok(()) => Ok(()),
                Err((error, pair)) => Err(Self {
                    diagnostics: diagnostics.followed_by(error.into()),
                    pair: Some(pair),
                    deadline,
                }),
            },
        }
    }
}
impl VethPair {
    pub fn create(
        parent: &Path,
        host: InterfaceName,
        peer: InterfaceName,
        deadline: Instant,
    ) -> Result<Self, VethFailure> {
        if host == peer {
            return Err(VethFailure::before(io::Error::other(
                "veth names must differ",
            )));
        }
        let namespace = File::open("/proc/self/ns/net").map_err(VethFailure::before)?;
        let parent = File::from(
            rustix::fs::open(
                parent,
                rustix::fs::OFlags::PATH
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::NOFOLLOW
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(|e| VethFailure::before(e.into()))?,
        );
        let metadata = parent.metadata().map_err(VethFailure::before)?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
            return Err(VethFailure::before(io::Error::other(
                "network control parent must be root-owned and protected",
            )));
        }
        let marker = tempfile::Builder::new()
            .prefix("conglobate-network-")
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))
            .map_err(VethFailure::before)?;
        let tag = marker
            .path()
            .file_name()
            .expect("generated marker name")
            .to_string_lossy()
            .into_owned();
        let mut host_address = Sha256::digest(tag.as_bytes())[..6].to_vec();
        host_address[0] = (host_address[0] & 0xfc) | 2;
        let mut peer_address = host_address.clone();
        peer_address[5] ^= 1;
        let pair = Self {
            host,
            peer,
            host_address,
            peer_address,
            namespace,
            marker: ManuallyDrop::new(marker),
            _parent: parent,
        };
        let result = pair.acquire(deadline);
        match result {
            Ok(()) => Ok(pair),
            Err(error) => Err(VethFailure::retained(
                error,
                Box::new(pair),
                Instant::now() + Duration::from_secs(30),
            )),
        }
    }
    /// Move the peer into a fresh command network namespace. Both the namespace
    /// and pair stay owned through failed setup and explicit cleanup.
    pub fn connect(
        self,
        namespace: NetworkNamespace,
        deadline: Instant,
    ) -> Result<ConnectedVeth, ConnectionFailure> {
        let result = (|| -> io::Result<()> {
            self.verify_namespace()?;
            let peer = self
                .lookup(&self.peer, deadline)?
                .ok_or_else(|| io::Error::other("veth peer disappeared before handoff"))?;
            if !self.owns(&peer) {
                return Err(io::Error::other("veth peer ownership changed"));
            }
            let mut request = LinkMessage::default();
            request.header.index = peer.header.index;
            request
                .attributes
                .push(LinkAttribute::NetNsFd(namespace.descriptor().as_raw_fd()));
            RouteRequest::new(RouteNetlinkMessage::NewLink(request), 0).execute(deadline)?;
            Ok(())
        })();
        match result {
            Ok(()) => Ok(ConnectedVeth {
                pair: self,
                namespace,
            }),
            Err(error) => Err(ConnectionFailure {
                failure: VethFailure::retained(
                    error,
                    Box::new(self),
                    Instant::now() + Duration::from_secs(30),
                ),
                namespace,
            }),
        }
    }
    pub fn host_name(&self) -> &InterfaceName {
        &self.host
    }
    pub fn peer_name(&self) -> &InterfaceName {
        &self.peer
    }
    pub fn close(self) -> Result<(), VethFailure> {
        let deadline = Instant::now() + Duration::from_secs(30);
        Box::new(self)
            .release(deadline)
            .map_err(|(error, pair)| VethFailure::retained(error, pair, deadline))
    }
    fn verify_namespace(&self) -> io::Result<()> {
        let expected = self.namespace.metadata()?;
        let actual = std::fs::metadata("/proc/self/ns/net")?;
        if (expected.dev(), expected.ino()) != (actual.dev(), actual.ino()) {
            return Err(io::Error::other(
                "veth operation in a different network namespace",
            ));
        }
        Ok(())
    }
    fn acquire(&self, deadline: Instant) -> io::Result<()> {
        self.verify_namespace()?;
        let mut peer = LinkMessage::default();
        peer.attributes = vec![
            LinkAttribute::IfName(self.peer.0.clone()),
            LinkAttribute::Address(self.peer_address.clone()),
        ];
        let mut host = LinkMessage::default();
        host.attributes = vec![
            LinkAttribute::IfName(self.host.0.clone()),
            LinkAttribute::Address(self.host_address.clone()),
            LinkAttribute::LinkInfo(vec![
                LinkInfo::Kind(InfoKind::Veth),
                LinkInfo::Data(InfoData::Veth(InfoVeth::Peer(peer))),
            ]),
        ];
        RouteRequest::new(
            RouteNetlinkMessage::NewLink(host),
            NLM_F_CREATE | NLM_F_EXCL,
        )
        .execute(deadline)?;
        for name in [&self.host, &self.peer] {
            let link = self
                .lookup(name, deadline)?
                .ok_or_else(|| io::Error::other("created veth disappeared"))?;
            if !self.owns(&link) {
                return Err(io::Error::other("created veth ownership tag mismatch"));
            }
        }
        Ok(())
    }
    fn lookup(&self, name: &InterfaceName, deadline: Instant) -> io::Result<Option<LinkMessage>> {
        let mut request = LinkMessage::default();
        request
            .attributes
            .push(LinkAttribute::IfName(name.0.clone()));
        let replies = match RouteRequest::new(RouteNetlinkMessage::GetLink(request), 0)
            .execute(deadline)
        {
            Ok(replies) => replies,
            Err(error) if error.raw_os_error() == Some(rustix::io::Errno::NODEV.raw_os_error()) => {
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let mut links = replies.into_iter().filter_map(|reply| {
            if let RouteNetlinkMessage::NewLink(link) = reply {
                Some(link)
            } else {
                None
            }
        });
        let link = links
            .next()
            .ok_or_else(|| io::Error::other("missing link reply"))?;
        if links.next().is_some()
            || !link
                .attributes
                .iter()
                .any(|a| matches!(a, LinkAttribute::IfName(n) if n == &name.0))
        {
            return Err(io::Error::other("ambiguous link reply"));
        }
        Ok(Some(link))
    }
    fn owns(&self, link: &LinkMessage) -> bool {
        link.attributes
            .iter()
            .any(|a| matches!(a, LinkAttribute::Address(address) if address == &self.host_address || address == &self.peer_address))
    }
    fn release(self: Box<Self>, deadline: Instant) -> Result<(), (io::Error, Box<Self>)> {
        let result = (|| -> io::Result<()> {
            self.verify_namespace()?;
            for name in [&self.host, &self.peer] {
                if let Some(link) = self.lookup(name, deadline)? {
                    // A failed exclusive create may have encountered somebody
                    // else's interface. Its name/index never grants deletion rights.
                    if !self.owns(&link) {
                        continue;
                    }
                    let mut request = LinkMessage::default();
                    request.header.index = link.header.index;
                    RouteRequest::new(RouteNetlinkMessage::DelLink(request), 0)
                        .execute(deadline)?;
                }
            }
            std::fs::remove_dir(self.marker.path())?;
            Ok(())
        })();
        match result {
            Err(error) => Err((error, self)),
            Ok(()) => {
                let Self { marker, .. } = *self;
                drop(ManuallyDrop::into_inner(marker));
                Ok(())
            }
        }
    }
}

/// A pair whose peer lives in its owned command network namespace. Commands
/// must be quiescent before close; this resource alone does not prove quiescence.
#[derive(Debug)]
pub struct ConnectedVeth {
    pair: VethPair,
    namespace: NetworkNamespace,
}
impl ConnectedVeth {
    pub fn close(self) -> Result<(), ConnectionFailure> {
        self.pair.close().map_err(|failure| ConnectionFailure {
            failure,
            namespace: self.namespace,
        })
    }
}
#[derive(Debug, thiserror::Error)]
#[error("network connection: {failure}")]
pub struct ConnectionFailure {
    failure: VethFailure,
    namespace: NetworkNamespace,
}
impl ConnectionFailure {
    pub fn cleanup(self) -> Result<(), Self> {
        self.failure.cleanup().map_err(|failure| Self {
            failure,
            namespace: self.namespace,
        })
    }
}
