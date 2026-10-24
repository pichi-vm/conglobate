// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::FormatterRuntime;
#[cfg(test)]
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
use rustix::{
    fs::{self, FileType, Mode},
    mount::{self, FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags, OpenTreeFlags},
};
use std::{
    fs::File,
    io,
    os::{
        fd::{AsFd, OwnedFd},
        unix::net::UnixDatagram,
    },
    process::Stdio,
};
#[cfg(test)]
use std::{io::IoSlice, mem::MaybeUninit};

/// Private descriptor packet for trusted formatter setup, never image commands.
#[derive(Debug)]
pub(crate) struct FormatterInput {
    root: File,
    devices: File,
}
impl FormatterInput {
    pub(super) fn new(runtime: &FormatterRuntime, device: devmap_linux::DevId) -> io::Result<Self> {
        // A separate clone prevents helper submounts from changing the parent's
        // read-only runtime view. All backing remains retained by that owner.
        let root = mount::open_tree(
            &runtime.root,
            "",
            OpenTreeFlags::AT_EMPTY_PATH
                | OpenTreeFlags::OPEN_TREE_CLONE
                | OpenTreeFlags::OPEN_TREE_CLOEXEC,
        )?
        .into();
        let context = mount::fsopen("tmpfs", FsOpenFlags::FSOPEN_CLOEXEC)?;
        mount::fsconfig_set_string(&context, "mode", "0700")?;
        mount::fsconfig_create(&context)?;
        let devices: File = mount::fsmount(
            &context,
            FsMountFlags::FSMOUNT_CLOEXEC,
            MountAttrFlags::MOUNT_ATTR_NOSUID | MountAttrFlags::MOUNT_ATTR_NOEXEC,
        )?
        .into();
        for (name, major, minor) in [("null", 1, 3), ("random", 1, 8), ("urandom", 1, 9)] {
            fs::mknodat(
                &devices,
                name,
                FileType::CharacterDevice,
                Mode::from_raw_mode(0o600),
                fs::makedev(major, minor),
            )?;
        }
        fs::mknodat(
            &devices,
            "output",
            FileType::BlockDevice,
            Mode::from_raw_mode(0o600),
            fs::makedev(device.major(), device.minor()),
        )?;
        Ok(Self { root, devices })
    }
    pub(super) fn into_stdio(self) -> io::Result<Stdio> {
        Ok(OwnedFd::from(self.into_socket()?).into())
    }
    fn into_socket(self) -> io::Result<UnixDatagram> {
        let (sender, receiver) = UnixDatagram::pair()?;
        let descriptors = [self.root.as_fd(), self.devices.as_fd()];
        crate::packet::DescriptorPacket::send_now(&sender, &[0x82, 1, 0], &descriptors)?;
        Ok(receiver)
    }
    pub(crate) fn from_stdin() -> io::Result<Self> {
        let socket = io::stdin().as_fd().try_clone_to_owned()?;
        Self::from_socket(&socket)
    }
    fn from_socket(socket: &impl AsFd) -> io::Result<Self> {
        let mut bytes = [0u8; 4];
        let received = crate::packet::DescriptorPacket::receive_now(socket, &mut bytes)?;
        let mut descriptors = received.descriptors;
        if received.bytes != 3 || bytes[..3] != [0x82, 1, 0] || descriptors.len() != 2 {
            return Err(io::Error::other("invalid formatter input"));
        }
        let devices: File = descriptors.pop().expect("checked count").into();
        let root: File = descriptors.pop().expect("checked count").into();
        if !root.metadata()?.is_dir()
            || !devices.metadata()?.is_dir()
            || fs::fstatfs(&devices)?.f_type != 0x0102_1994
        {
            return Err(io::Error::other("invalid formatter mounts"));
        }
        Ok(Self { root, devices })
    }
    /// Dedicated trusted helper only; any failure is exit-only. The caller
    /// unshares its mount namespace before consuming these detached mounts.
    pub(crate) fn enter(self) -> io::Result<()> {
        mount::move_mount(
            &self.devices,
            "",
            &self.root,
            "dev",
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
        )?;
        rustix::process::fchdir(&self.root)?;
        rustix::process::chroot(".")?;
        rustix::process::chdir("/")?;
        let null = File::open("/dev/null")?;
        rustix::stdio::dup2_stdin(&null)?;
        // All input FDs close here. No directory outside the new root survives.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        os::unix::{fs::MetadataExt, net::UnixStream},
        time::Duration,
    };
    struct Fixture;
    impl Fixture {
        fn send(socket: &impl AsFd, bytes: &[u8], fds: &[std::os::fd::BorrowedFd<'_>]) {
            let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(32))];
            let mut ancillary = SendAncillaryBuffer::new(&mut storage);
            if !fds.is_empty() {
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(fds)));
            }
            rustix::net::sendmsg(
                socket,
                &[IoSlice::new(bytes)],
                &mut ancillary,
                SendFlags::DONTWAIT,
            )
            .unwrap();
        }
        fn reject(bytes: &[u8], count: usize) {
            let (tx, rx) = UnixDatagram::pair().unwrap();
            let (owners, peers): (Vec<_>, Vec<_>) =
                (0..count).map(|_| UnixStream::pair().unwrap()).unzip();
            Self::send(
                &tx,
                bytes,
                &owners.iter().map(AsFd::as_fd).collect::<Vec<_>>(),
            );
            drop(owners);
            assert!(FormatterInput::from_socket(&rx).is_err());
            for mut peer in peers {
                peer.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                assert_eq!(peer.read(&mut [0]).unwrap(), 0);
            }
        }
    }
    #[test]
    fn malformed_or_truncated_packets_release_rights() {
        for bytes in [
            &[][..],
            &[0x82, 1],
            &[0x82, 2, 0],
            &[0x82, 1, 1],
            &[0x82, 1, 0, 0],
            &[0x82, 1, 0, 0, 0],
        ] {
            Fixture::reject(bytes, 2);
        }
        for count in [0, 1, 2, 3, 8, 32] {
            Fixture::reject(&[0x82, 1, 0], count);
        }
        let (tx, rx) = UnixDatagram::pair().unwrap();
        let proc = File::open("/proc").unwrap();
        Fixture::send(&tx, &[0x82, 1, 0], &[proc.as_fd(), proc.as_fd()]);
        assert!(FormatterInput::from_socket(&rx).is_err());
    }
    #[test]
    fn input_requires_a_queued_offer() {
        let (_tx, rx) = UnixDatagram::pair().unwrap();
        assert_eq!(
            FormatterInput::from_socket(&rx).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn production_handoff_preserves_identity_and_cloexec() {
        let directory = tempfile::tempdir_in("/dev/shm").unwrap();
        let root = File::open(directory.path()).unwrap();
        let inode = root.metadata().unwrap().ino();
        let input = FormatterInput {
            devices: root.try_clone().unwrap(),
            root,
        };
        let socket = input.into_socket().unwrap();
        let received = FormatterInput::from_socket(&socket).unwrap();
        drop(socket);
        for file in [received.root, received.devices] {
            assert_eq!(file.metadata().unwrap().ino(), inode);
            assert!(
                rustix::io::fcntl_getfd(&file)
                    .unwrap()
                    .contains(rustix::io::FdFlags::CLOEXEC)
            );
        }
    }
}
