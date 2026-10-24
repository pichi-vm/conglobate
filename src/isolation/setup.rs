// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

mod gated;
use super::{CommandDevices, CommandNamespaces};
pub(crate) use gated::{GatedCommandSetup, GatedRootSetup};
use rustix::fd::{AsFd, OwnedFd};
#[cfg(test)]
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};
use std::{fs::File, io, os::unix::net::UnixDatagram, process::Stdio};
#[cfg(test)]
use std::{io::IoSlice, mem::MaybeUninit};

/// One-shot trusted setup input: the selected namespaces, private device mount
/// and optional read-only resolver view. No descriptor reaches image commands. This is not
/// an authenticated input format for an untrusted sender or command stream.
#[derive(Debug)]
pub(crate) struct CommandSetup {
    namespace: File,
    devices: CommandDevices,
    network: Option<File>,
    resolver: Option<crate::network::ResolverView>,
}
impl CommandSetup {
    pub(crate) fn new(namespace: File, devices: CommandDevices) -> io::Result<Self> {
        if rustix::fs::fstatfs(&namespace)?.f_type as u64 != 0x6e73_6673 {
            return Err(io::Error::other("setup namespace requires nsfs"));
        }
        Ok(Self {
            namespace,
            devices,
            network: None,
            resolver: None,
        })
    }
    pub(crate) fn with_resolver_view(
        self,
        resolver: crate::network::ResolverView,
    ) -> io::Result<Self> {
        if self.network.is_none() {
            return Err(io::Error::other(
                "resolver injection requires configured networking",
            ));
        }
        Ok(Self {
            resolver: Some(resolver),
            ..self
        })
    }
    pub(crate) fn with_network_handle(self, network: File) -> io::Result<Self> {
        if rustix::fs::fstatfs(&network)?.f_type as u64 != 0x6e73_6673 {
            return Err(io::Error::other("setup network namespace requires nsfs"));
        }
        Ok(Self {
            network: Some(network),
            ..self
        })
    }

    /// Enter only in a fresh trusted setup process. Partial namespace transition
    /// is exit-only. The remaining device owner must be passed to root setup;
    /// this does not authorize image execution or certify complete isolation.
    /// The caller supplies its existing setup deadline; entry never renews it.
    pub(crate) fn enter_until(
        self,
        deadline: std::time::Instant,
    ) -> io::Result<(CommandNamespaces, CommandDevices)> {
        if std::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "command setup deadline expired",
            ));
        }
        if let Some(resolver) = self.resolver {
            // Clone mounts while still initially privileged. Opening cwd after
            // cloning selects this namespace's mount, not the parent's view.
            // Scope restoration after a failed attach is exit-only; the helper
            // cannot proceed to image execution on any error.
            let scope = super::PrivateMountNamespace::enter()
                .map_err(|error| io::Error::other(error.to_string()))?;
            resolver.install(File::open(".")?)?;
            scope.commit();
        }
        Ok((
            CommandNamespaces::enter_with_network(self.namespace, self.network, deadline)?,
            self.devices,
        ))
    }
}
