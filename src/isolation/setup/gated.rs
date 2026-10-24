// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::*;
use crate::isolation::{CommandNamespaces, ExecutionEndpoint};
mod helper;

#[derive(Debug)]
pub(crate) struct GatedCommandSetup {
    setup: CommandSetup,
    gate: ExecutionEndpoint,
}
#[derive(Debug)]
pub(crate) struct GatedRootSetup {
    devices: CommandDevices,
    gate: ExecutionEndpoint,
}
impl CommandSetup {
    pub(crate) fn with_gate(self, gate: ExecutionEndpoint) -> GatedCommandSetup {
        GatedCommandSetup { setup: self, gate }
    }
}
impl GatedCommandSetup {
    pub(crate) fn into_input(self) -> io::Result<Stdio> {
        let mut descriptors = vec![
            self.setup.namespace.into(),
            self.setup.devices.into_handle().into(),
            self.gate.into_handle(),
        ];
        let mut opcode = if let Some(network) = self.setup.network {
            descriptors.push(network.into());
            5
        } else {
            2
        };
        if let Some(resolver) = self.setup.resolver {
            descriptors.push(resolver.into_handle().into());
            opcode = 7;
        }
        Packet {
            opcode,
            descriptors,
        }
        .send()
    }
    pub(crate) fn from_input() -> io::Result<Self> {
        let socket = io::stdin().as_fd().try_clone_to_owned()?;
        Self::from_socket(&socket)
    }
    fn from_socket(socket: &impl AsFd) -> io::Result<Self> {
        let mut packet = Packet::receive_shapes(socket, &[(2, 3), (5, 4), (7, 5)])?;
        let resolver = if packet.opcode == 7 {
            Some(crate::network::ResolverView::from_handle(
                packet.descriptors.pop().expect("resolver").into(),
            )?)
        } else {
            None
        };
        let network = if matches!(packet.opcode, 5 | 7) {
            Some(File::from(packet.descriptors.pop().expect("network")))
        } else {
            None
        };
        let gate = ExecutionEndpoint::from_handle(packet.descriptors.pop().expect("gate"))?;
        let devices =
            CommandDevices::from_handle(packet.descriptors.pop().expect("devices").into())?;
        let setup =
            CommandSetup::new(packet.descriptors.pop().expect("namespace").into(), devices)?;
        let setup = match network {
            Some(network) => setup.with_network_handle(network)?,
            None => setup,
        };
        let setup = match resolver {
            Some(resolver) => setup.with_resolver_view(resolver)?,
            None => setup,
        };
        Ok(Self { setup, gate })
    }
    /// Enter using the existing setup deadline, without starting a new budget.
    pub(crate) fn enter_until(
        self,
        deadline: std::time::Instant,
    ) -> io::Result<(CommandNamespaces, GatedRootSetup)> {
        let (scope, devices) = self.setup.enter_until(deadline)?;
        Ok((
            scope,
            GatedRootSetup {
                devices,
                gate: self.gate,
            },
        ))
    }
}
impl GatedRootSetup {
    pub(crate) fn into_input(self) -> io::Result<Stdio> {
        Packet {
            opcode: 3,
            descriptors: vec![self.devices.into_handle().into(), self.gate.into_handle()],
        }
        .send()
    }
    pub(crate) fn from_input() -> io::Result<Self> {
        Self::from_socket(&io::stdin())
    }
    fn from_socket(socket: &impl AsFd) -> io::Result<Self> {
        let mut packet = Packet::receive_shapes(socket, &[(3, 2)])?;
        let gate = ExecutionEndpoint::from_handle(packet.descriptors.pop().expect("gate"))?;
        let devices =
            CommandDevices::from_handle(packet.descriptors.pop().expect("devices").into())?;
        Ok(Self { devices, gate })
    }
    pub(crate) fn into_parts(self) -> (CommandDevices, ExecutionEndpoint) {
        (self.devices, self.gate)
    }
}
#[derive(Debug)]
struct Packet {
    opcode: u8,
    descriptors: Vec<OwnedFd>,
}
impl Packet {
    fn send(self) -> io::Result<Stdio> {
        let (sender, receiver) = UnixDatagram::pair()?;
        let descriptors: Vec<_> = self.descriptors.iter().map(AsFd::as_fd).collect();
        crate::packet::DescriptorPacket::send_now(&sender, &[0x82, 1, self.opcode], &descriptors)?;
        Ok(OwnedFd::from(receiver).into())
    }
    fn receive_shapes(socket: &impl AsFd, shapes: &[(u8, usize)]) -> io::Result<Self> {
        let mut bytes = [0; 4];
        let received = crate::packet::DescriptorPacket::receive_now(socket, &mut bytes)?;
        let descriptors = received.descriptors;
        if received.bytes != 3
            || bytes[..2] != [0x82, 1]
            || !shapes.contains(&(bytes[2], descriptors.len()))
        {
            return Err(io::Error::other("invalid gated setup packet"));
        }
        Ok(Self {
            opcode: bytes[2],
            descriptors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, os::unix::net::UnixStream, time::Duration};
    struct Fixture;
    impl Fixture {
        fn reject(bytes: &[u8], count: usize) {
            let (sender, receiver) = UnixDatagram::pair().unwrap();
            let (owners, peers): (Vec<_>, Vec<_>) =
                (0..count).map(|_| UnixStream::pair().unwrap()).unzip();
            let fds: Vec<_> = owners.iter().map(AsFd::as_fd).collect();
            let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(5))];
            let mut ancillary = SendAncillaryBuffer::new(&mut space);
            if count != 0 {
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(&fds)));
            }
            sendmsg(
                &sender,
                &[IoSlice::new(bytes)],
                &mut ancillary,
                SendFlags::NOSIGNAL,
            )
            .unwrap();
            drop(owners);
            assert!(Packet::receive_shapes(&receiver, &[(2, 3)]).is_err());
            for mut peer in peers {
                peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                assert_eq!(peer.read(&mut [0]).unwrap(), 0);
            }
        }
    }
    #[test]
    fn gated_setup_rejects_wrong_shapes_and_releases_all_rights() {
        for count in [0, 1, 2, 4, 5] {
            Fixture::reject(&[0x82, 1, 2], count);
        }
        for bytes in [
            &[][..],
            &[0x82, 1],
            &[0x82, 2, 2],
            &[0x82, 1, 3],
            &[0x82, 1, 2, 0],
        ] {
            Fixture::reject(bytes, 3);
        }
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;
    use crate::isolation::ExecutionGate;
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
            let (_gate, endpoint) = ExecutionGate::pair().unwrap();
            let endpoint = endpoint.into_handle();
            let mut owned_rights = rights.to_vec();
            if owned_rights.len() >= 2 {
                owned_rights.insert(2, endpoint.as_fd());
            }
            let rights = &owned_rights;
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
            assert!(GatedCommandSetup::from_socket(&transfer.receiver).is_err());
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
            Transfer::reject(&[0x82, 1, 2], count);
        }
        for count in [0, 1, 2, 3, 4, 8] {
            Transfer::reject(&[0x82, 1, 5], count);
            Transfer::reject(&[0x82, 1, 7], count);
        }
        let transfer = Transfer::new();
        let namespace = File::open("/proc/self/ns/user").unwrap();
        let proc = File::open("/proc").unwrap();
        transfer.send(&[0x82, 1, 2], &[namespace.as_fd(), proc.as_fd()]);
        assert!(GatedCommandSetup::from_socket(&transfer.receiver).is_err());
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
            &[0x82, 1, 7],
            &[
                user.as_fd(),
                devices.as_fd(),
                network.as_fd(),
                resolver.as_fd(),
            ],
        );
        assert!(
            GatedCommandSetup::from_socket(&transfer.receiver)
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
            &[0x82, 1, 5],
            &[user.as_fd(), devices.as_fd(), network.as_fd()],
        );
        let received = GatedCommandSetup::from_socket(&transfer.receiver).unwrap();
        let handle = received.setup.network.unwrap();
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
            GatedCommandSetup::from_socket(&transfer.receiver)
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
        transfer.send(&[0x82, 1, 2], &[namespace.as_fd(), devices.as_fd()]);
        let received = GatedCommandSetup::from_socket(&transfer.receiver).unwrap();
        assert_eq!(received.setup.namespace.metadata().unwrap().ino(), expected);
        assert!(
            rustix::io::fcntl_getfd(&received.setup.namespace)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let device = received.setup.devices.into_handle();
        assert!(
            rustix::io::fcntl_getfd(&device)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
        let bad = File::open(directory.path()).unwrap();
        assert!(CommandSetup::new(bad, CommandDevices::from_handle(device).unwrap()).is_err());
    }
}
