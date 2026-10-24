// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags, recvmsg, sendmsg,
};
use rustix::{
    fs::{self, AtFlags, FileType, Mode},
    mount::{self, FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags},
    process::{Gid, Uid},
};
use std::{
    fs::File,
    io::{self},
};
#[cfg(test)]
use std::{
    io::{IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::{fd::AsFd, unix::net::UnixDatagram},
};

/// A fresh detached tmpfs containing only approved private device inodes.
/// Create in the trusted initial-user-namespace helper before joining the
/// command user namespace. The owner IDs are that command's allocated mapped
/// root IDs. No supervisor /dev inode is reused.
#[derive(Debug)]
pub(crate) struct CommandDevices {
    root: File,
}
impl CommandDevices {
    #[cfg(test)]
    fn receive(socket: &impl AsFd) -> io::Result<Self> {
        let mut bytes = [0u8; 4];
        let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut storage);
        let received = recvmsg(
            socket,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT,
        )?;
        let mut descriptors = Vec::new();
        for item in ancillary.drain() {
            match item {
                RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
                _ => {
                    return Err(io::Error::other(
                        "unexpected device transfer ancillary message",
                    ));
                }
            }
        }
        if received.bytes != 3
            || bytes[..3] != [0x82, 1, 0]
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || descriptors.len() != 1
        {
            return Err(io::Error::other("invalid device transfer packet"));
        }
        let root: File = descriptors
            .pop()
            .expect("checked single device descriptor")
            .into();
        Self::from_handle(root)
    }
    #[cfg(test)]
    fn into_channel(self) -> io::Result<UnixDatagram> {
        // Queue the original mount FD, not a reopened directory: fsmount's
        // ownership must survive until attachment. A socket is also valid stdin
        // for language-runtime standard-stream sanitization (unlike O_PATH).
        let (sender, receiver) = UnixDatagram::pair()?;
        let descriptors = [self.root.as_fd()];
        let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut storage);
        if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(io::Error::other("device transfer ancillary capacity"));
        }
        let sent = sendmsg(
            &sender,
            &[IoSlice::new(&[0x82, 1, 0])],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )?;
        if sent != 3 {
            return Err(io::Error::other("incomplete device transfer"));
        }
        Ok(receiver)
    }
    pub(super) fn into_handle(self) -> File {
        self.root
    }
    pub(super) fn from_handle(root: File) -> io::Result<Self> {
        if !root.metadata()?.is_dir() || fs::fstatfs(&root)?.f_type != 0x0102_1994 {
            return Err(io::Error::other(
                "private device input must be a tmpfs directory",
            ));
        }
        Ok(Self { root })
    }
    pub(crate) fn create(uid: u32, gid: u32) -> io::Result<Self> {
        if uid == u32::MAX || gid == u32::MAX {
            return Err(io::Error::other("invalid runtime device owner"));
        }
        let context = mount::fsopen("tmpfs", FsOpenFlags::FSOPEN_CLOEXEC)?;
        mount::fsconfig_set_string(&context, "uid", uid.to_string())?;
        mount::fsconfig_set_string(&context, "gid", gid.to_string())?;
        mount::fsconfig_set_string(&context, "mode", "0755")?;
        mount::fsconfig_create(&context)?;
        // Device access is intentional only on this private tmpfs. The image
        // mount itself remains nodev; no block/control devices are created.
        let root: File = mount::fsmount(
            &context,
            FsMountFlags::FSMOUNT_CLOEXEC,
            MountAttrFlags::MOUNT_ATTR_NOSUID | MountAttrFlags::MOUNT_ATTR_NOEXEC,
        )?
        .into();
        for (name, major, minor) in [
            ("null", 1, 3),
            ("zero", 1, 5),
            ("full", 1, 7),
            ("random", 1, 8),
            ("urandom", 1, 9),
            ("tty", 5, 0),
        ] {
            fs::mknodat(
                &root,
                name,
                FileType::CharacterDevice,
                Mode::from_raw_mode(0o666),
                fs::makedev(major, minor),
            )?;
            fs::chownat(
                &root,
                name,
                Some(Uid::from_raw(uid)),
                Some(Gid::from_raw(gid)),
                AtFlags::empty(),
            )?;
            fs::chmodat(&root, name, Mode::from_raw_mode(0o666), AtFlags::empty())?;
        }
        for name in ["pts", "shm"] {
            fs::mkdirat(&root, name, Mode::from_raw_mode(0o755))?;
            fs::chownat(
                &root,
                name,
                Some(Uid::from_raw(uid)),
                Some(Gid::from_raw(gid)),
                AtFlags::empty(),
            )?;
        }
        for (name, target) in [
            ("ptmx", "pts/ptmx"),
            ("fd", "/proc/self/fd"),
            ("stdin", "/proc/self/fd/0"),
            ("stdout", "/proc/self/fd/1"),
            ("stderr", "/proc/self/fd/2"),
        ] {
            fs::symlinkat(target, &root, name)?;
        }
        Ok(Self { root })
    }

    pub(super) fn attach(self, target: &File) -> io::Result<()> {
        mount::move_mount(
            &self.root,
            "",
            target,
            "",
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )?;
        DeviceRuntime::Pts.attach(&self.root, "pts")?;
        DeviceRuntime::SharedMemory.attach(&self.root, "shm")
    }
}

#[derive(Debug)]
enum DeviceRuntime {
    Pts,
    SharedMemory,
}
impl DeviceRuntime {
    fn attach(self, root: &File, name: &str) -> io::Result<()> {
        // These directories were freshly created by trusted setup, not read
        // from the image. Still resolve without symlinks or mount crossings.
        let target = fs::openat2(
            root,
            name,
            fs::OFlags::PATH | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC,
            Mode::empty(),
            fs::ResolveFlags::BENEATH | fs::ResolveFlags::NO_SYMLINKS | fs::ResolveFlags::NO_XDEV,
        )?;
        let context = mount::fsopen(
            match self {
                Self::Pts => "devpts",
                Self::SharedMemory => "tmpfs",
            },
            FsOpenFlags::FSOPEN_CLOEXEC,
        )?;
        let mut flags = MountAttrFlags::MOUNT_ATTR_NOSUID | MountAttrFlags::MOUNT_ATTR_NOEXEC;
        match self {
            Self::Pts => {
                mount::fsconfig_set_flag(&context, "newinstance")?;
                mount::fsconfig_set_string(&context, "ptmxmode", "0666")?;
                mount::fsconfig_set_string(&context, "mode", "0620")?;
                mount::fsconfig_set_string(&context, "gid", "0")?;
            }
            Self::SharedMemory => {
                mount::fsconfig_set_string(&context, "mode", "1777")?;
                flags |= MountAttrFlags::MOUNT_ATTR_NODEV;
            }
        }
        mount::fsconfig_create(&context)?;
        let tree = mount::fsmount(&context, FsMountFlags::FSMOUNT_CLOEXEC, flags)?;
        mount::move_mount(
            &tree,
            "",
            &target,
            "",
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Read,
        os::{fd::BorrowedFd, unix::net::UnixStream},
    };

    #[derive(Debug)]
    struct Transfer {
        sender: UnixDatagram,
        receiver: UnixDatagram,
    }
    impl Transfer {
        fn new() -> Self {
            let (sender, receiver) = UnixDatagram::pair().unwrap();
            Self { sender, receiver }
        }

        fn send(&self, bytes: &[u8], descriptors: &[BorrowedFd<'_>]) {
            let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(4))];
            let mut ancillary = SendAncillaryBuffer::new(&mut storage);
            if !descriptors.is_empty() {
                assert!(ancillary.push(SendAncillaryMessage::ScmRights(descriptors)));
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

        fn reject_and_check_closure(bytes: &[u8], count: usize) {
            let transfer = Self::new();
            let (owners, peers): (Vec<_>, Vec<_>) =
                (0..count).map(|_| UnixStream::pair().unwrap()).unzip();
            transfer.send(bytes, &owners.iter().map(AsFd::as_fd).collect::<Vec<_>>());
            drop(owners);
            assert!(CommandDevices::receive(&transfer.receiver).is_err());
            // EOF proves received/truncated rights were closed. This avoids
            // racing other tests by counting process-wide numeric descriptors.
            for mut peer in peers {
                // Concurrent process spawning can briefly duplicate CLOEXEC
                // rights until exec. Await EOF under a bound, not a scheduling
                // assumption; a leaked receiver reference still fails the test.
                peer.set_read_timeout(Some(std::time::Duration::from_secs(5)))
                    .unwrap();
                assert_eq!(peer.read(&mut [0u8; 1]).unwrap(), 0);
            }
        }
    }

    #[test]
    fn malformed_or_truncated_device_packets_release_rights() {
        for bytes in [
            &[][..],
            &[0x82, 1],
            &[0x82, 2, 0],
            &[0x82, 1, 1],
            &[0x82, 1, 0, 0],
            &[0x82, 1, 0, 0, 0],
        ] {
            Transfer::reject_and_check_closure(bytes, 1);
        }
        for count in [0, 2, 3, 4] {
            Transfer::reject_and_check_closure(&[0x82, 1, 0], count);
        }
        // Valid framing with a non-directory FD must reject and release it too.
        Transfer::reject_and_check_closure(&[0x82, 1, 0], 1);
        let transfer = Transfer::new();
        let proc = File::open("/proc").unwrap();
        transfer.send(&[0x82, 1, 0], &[proc.as_fd()]);
        assert!(CommandDevices::receive(&transfer.receiver).is_err());
    }

    #[test]
    fn device_receiver_does_not_wait_for_an_unqueued_offer() {
        let transfer = Transfer::new();
        assert_eq!(
            CommandDevices::receive(&transfer.receiver)
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
    }

    #[test]
    fn device_handoff_keeps_cloexec_and_independent_ownership() {
        // Ordinary unprivileged tmpfs directory, not device construction or a
        // mount test. The production sender/receiver transport is exercised.
        let directory = tempfile::tempdir_in("/dev/shm").unwrap();
        let source = CommandDevices {
            root: File::open(directory.path()).unwrap(),
        };
        let socket = source.into_channel().unwrap();
        let received = CommandDevices::receive(&socket).unwrap();
        drop(socket);
        assert!(received.root.metadata().unwrap().is_dir());
        assert!(
            rustix::io::fcntl_getfd(&received.root)
                .unwrap()
                .contains(rustix::io::FdFlags::CLOEXEC)
        );
    }
}
