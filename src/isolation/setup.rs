// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

mod gated;
use super::{CommandDevices, CommandNamespaces};
pub use gated::{
    GatedCommandSetup, GatedRestrictedCommand, GatedRootSetup, HelperExecutionError,
    RootPreparationError,
};
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

/// One-shot trusted setup input: the selected namespaces, private device mount
/// and optional read-only resolver view. No descriptor reaches image commands. This is not
/// an authenticated input format for an untrusted sender or command stream.
#[derive(Debug)]
pub struct CommandSetup {
    namespace: File,
    devices: CommandDevices,
    network: Option<File>,
    resolver: Option<crate::network::ResolverView>,
}
impl CommandSetup {
    pub fn new(namespace: File, devices: CommandDevices) -> io::Result<Self> {
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
    /// Select a supervisor-owned network namespace. The enclosing network
    /// resource must remain owned until all command descendants are quiescent.
    pub fn with_network(self, namespace: &crate::network::NetworkNamespace) -> io::Result<Self> {
        self.with_network_handle(namespace.descriptor().try_clone()?)
    }
    /// Source and image scaffolding remain owned outside this helper through
    /// command quiescence. Only configured-network steps accept DNS injection.
    pub fn with_resolver(self, source: &crate::network::ResolverSource) -> io::Result<Self> {
        self.with_resolver_view(source.view()?)
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
    pub fn namespace_metadata(&self) -> io::Result<Metadata> {
        self.namespace.metadata()
    }

    pub fn into_input(self) -> io::Result<Stdio> {
        let (sender, receiver) = UnixDatagram::pair()?;
        let device = self.devices.into_handle();
        let mut descriptors = vec![self.namespace.as_fd(), device.as_fd()];
        let mut opcode = if let Some(network) = &self.network {
            descriptors.push(network.as_fd());
            4
        } else {
            1
        };
        if let Some(resolver) = &self.resolver {
            descriptors.push(resolver.handle().as_fd());
            opcode = 6;
        }
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(io::Error::other("setup transfer ancillary capacity"));
        }
        if sendmsg(
            &sender,
            &[IoSlice::new(&[0x82, 1, opcode])],
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
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(5))];
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
            || bytes[..2] != [0x82, 1]
            || !matches!((bytes[2], descriptors.len()), (1, 2) | (4, 3) | (6, 4))
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
        {
            return Err(io::Error::other("invalid setup transfer packet"));
        }
        let resolver = if bytes[2] == 6 {
            Some(crate::network::ResolverView::from_handle(
                descriptors.pop().expect("resolver").into(),
            )?)
        } else {
            None
        };
        let network = if matches!(bytes[2], 4 | 6) {
            Some(File::from(descriptors.pop().expect("network")))
        } else {
            None
        };
        let devices = CommandDevices::from_handle(
            descriptors.pop().expect("checked device descriptor").into(),
        )?;
        let setup = Self::new(
            descriptors
                .pop()
                .expect("checked namespace descriptor")
                .into(),
            devices,
        )?;
        let setup = match network {
            Some(network) => setup.with_network_handle(network)?,
            None => setup,
        };
        match resolver {
            Some(resolver) => setup.with_resolver_view(resolver),
            None => Ok(setup),
        }
    }

    /// Enter only in a fresh trusted setup process. Partial namespace transition
    /// is exit-only. The remaining device owner must be passed to root setup;
    /// this does not authorize image execution or certify complete isolation.
    pub fn enter(self) -> io::Result<(CommandNamespaces, CommandDevices)> {
        self.enter_until(std::time::Instant::now() + std::time::Duration::from_secs(30))
    }
    pub fn enter_until(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        os::{
            fd::BorrowedFd,
            unix::{fs::MetadataExt, net::UnixStream},
        },
    };

    struct Transfer {
        sender: UnixDatagram,
        receiver: UnixDatagram,
    }
    impl Transfer {
        fn new() -> Self {
            let (sender, receiver) = UnixDatagram::pair().unwrap();
            Self { sender, receiver }
        }
        fn send(&self, bytes: &[u8], rights: &[BorrowedFd<'_>]) {
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(8))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if !rights.is_empty() {
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(rights)));
            }
            assert_eq!(
                sendmsg(
                    &self.sender,
                    &[IoSlice::new(bytes)],
                    &mut ancillary,
                    SendFlags::DONTWAIT | SendFlags::NOSIGNAL
                )
                .unwrap(),
                bytes.len()
            );
        }
        fn reject(bytes: &[u8], count: usize) {
            let transfer = Self::new();
            let (owners, peers): (Vec<_>, Vec<_>) =
                (0..count).map(|_| UnixStream::pair().unwrap()).unzip();
            transfer.send(bytes, &owners.iter().map(AsFd::as_fd).collect::<Vec<_>>());
            drop(owners);
            assert!(CommandSetup::receive(&transfer.receiver).is_err());
            for mut peer in peers {
                peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                assert_eq!(peer.read(&mut [0]).unwrap(), 0);
            }
        }
    }
    #[test]
    fn invalid_packets_and_object_types_release_all_rights() {
        for bytes in [
            &[][..],
            &[0x82, 1],
            &[0x82, 2, 1],
            &[0x82, 1, 0],
            &[0x82, 1, 1, 0],
            &[0x82, 1, 1, 0, 0],
        ] {
            Transfer::reject(bytes, 2);
        }
        for count in [0, 1, 2, 3, 8] {
            Transfer::reject(&[0x82, 1, 1], count);
        }
        for count in [0, 1, 2, 3, 4, 8] {
            Transfer::reject(&[0x82, 1, 4], count);
            Transfer::reject(&[0x82, 1, 6], count);
        }
        let transfer = Transfer::new();
        let namespace = File::open("/proc/self/ns/user").unwrap();
        let proc = File::open("/proc").unwrap();
        transfer.send(&[0x82, 1, 1], &[namespace.as_fd(), proc.as_fd()]);
        assert!(CommandSetup::receive(&transfer.receiver).is_err());
    }
    #[test]
    fn resolver_transport_rejects_a_writable_tmpfs_file() {
        let directory = tempfile::tempdir_in("/dev/shm").unwrap();
        let resolver = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        let user = File::open("/proc/self/ns/user").unwrap();
        let network = File::open("/proc/self/ns/net").unwrap();
        let devices = File::open(directory.path()).unwrap();
        let transfer = Transfer::new();
        transfer.send(
            &[0x82, 1, 6],
            &[
                user.as_fd(),
                devices.as_fd(),
                network.as_fd(),
                resolver.as_fd(),
            ],
        );
        assert!(
            CommandSetup::receive(&transfer.receiver)
                .unwrap_err()
                .to_string()
                .contains("read-only")
        );
    }
    #[test]
    fn network_transport_retains_identity_and_close_on_exec() {
        let directory = tempfile::tempdir_in("/dev/shm").unwrap();
        let user = File::open("/proc/self/ns/user").unwrap();
        let network = File::open("/proc/self/ns/net").unwrap();
        let devices = File::open(directory.path()).unwrap();
        let transfer = Transfer::new();
        transfer.send(
            &[0x82, 1, 4],
            &[user.as_fd(), devices.as_fd(), network.as_fd()],
        );
        let received = CommandSetup::receive(&transfer.receiver).unwrap();
        let handle = received.network.unwrap();
        assert_eq!(
            handle.metadata().unwrap().ino(),
            network.metadata().unwrap().ino()
        );
        assert!(
            rustix::io::fcntl_getfd(&handle)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
    #[test]
    fn receiver_requires_a_queued_offer() {
        let transfer = Transfer::new();
        assert_eq!(
            CommandSetup::receive(&transfer.receiver)
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn transport_preserves_identity_and_sets_close_on_exec() {
        // Ordinary existing tmpfs directory, not a privileged device mount.
        // This tests transport only; mount authority is exercised in the VM.
        let directory = tempfile::tempdir_in("/dev/shm").unwrap();
        let namespace = File::open("/proc/self/ns/user").unwrap();
        let expected = namespace.metadata().unwrap().ino();
        let devices = File::open(directory.path()).unwrap();
        let transfer = Transfer::new();
        transfer.send(&[0x82, 1, 1], &[namespace.as_fd(), devices.as_fd()]);
        let received = CommandSetup::receive(&transfer.receiver).unwrap();
        assert_eq!(received.namespace_metadata().unwrap().ino(), expected);
        assert!(
            rustix::io::fcntl_getfd(&received.namespace)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let device = received.devices.into_handle();
        assert!(
            rustix::io::fcntl_getfd(&device)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let bad = File::open(directory.path()).unwrap();
        assert!(CommandSetup::new(bad, CommandDevices::from_handle(device).unwrap()).is_err());
    }
}
