// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::netfilter::NetfilterSocket;
use netlink_packet_core::{NLM_F_CREATE, NLM_F_EXCL};
use netlink_packet_netfilter::nftables::{
    NfTablesMessage, TableAttribute, TableFlags, TableMessage,
};
use std::{io, time::Instant};

/// A kernel socket-owned nftables table. Dropping the socket releases the table;
/// explicit close also requires an acknowledged deletion. No image descriptor
/// receives this socket. An empty table does not implement a firewall policy.
#[derive(Debug)]
pub struct FirewallTable {
    socket: NetfilterSocket,
    name: String,
}
#[derive(Debug, thiserror::Error)]
#[error("firewall table teardown: {source}")]
pub struct FirewallCloseFailure {
    source: crate::storage::FailureDiagnostics,
    table: FirewallTable,
    deadline: Instant,
}
impl FirewallCloseFailure {
    pub(crate) fn diagnostics(&self) -> &crate::storage::FailureDiagnostics {
        &self.source
    }
    pub fn cleanup(self) -> Result<(), Self> {
        self.table.close(self.deadline).map_err(|failure| Self {
            source: self.source.followed_by(failure.source),
            table: failure.table,
            deadline: failure.deadline,
        })
    }
}
impl FirewallTable {
    pub fn create(name: &str, deadline: Instant) -> io::Result<Self> {
        Self::install(name, Vec::new(), deadline)
    }
    pub(super) fn install(
        name: &str,
        extra: Vec<(NfTablesMessage, u16)>,
        deadline: Instant,
    ) -> io::Result<Self> {
        if name.is_empty()
            || name.len() > 64
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid private firewall table name",
            ));
        }
        let mut socket = NetfilterSocket::new()?;
        let mut requests = vec![(
            NfTablesMessage::NewTable(TableMessage {
                attributes: vec![
                    TableAttribute::Name(name.into()),
                    TableAttribute::Flags(TableFlags::Owner),
                ],
            }),
            NLM_F_CREATE | NLM_F_EXCL,
        )];
        requests.extend(extra);
        socket.transact(requests, deadline)?;
        Ok(Self {
            socket,
            name: name.into(),
        })
    }
    pub fn close(mut self, deadline: Instant) -> Result<(), FirewallCloseFailure> {
        let request = NfTablesMessage::DeleteTable(TableMessage {
            attributes: vec![TableAttribute::Name(self.name.clone())],
        });
        match self.socket.transact(vec![(request, 0)], deadline) {
            Ok(()) => Ok(()),
            Err(source) => Err(FirewallCloseFailure {
                source: source.into(),
                table: self,
                deadline,
            }),
        }
    }
}
