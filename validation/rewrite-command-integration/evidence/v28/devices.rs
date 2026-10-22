// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

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
    io::{self, IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::{fd::AsFd, unix::net::UnixDatagram},
    process::Stdio,
};

/// A fresh detached tmpfs containing only approved private device inodes.
/// Create in the trusted initial-user-namespace helper before joining the
/// command user namespace. The owner IDs are that command's allocated mapped
/// root IDs. No supervisor /dev inode is reused.
#[derive(Debug)]
pub struct CommandDevices {
    root: File,
}
impl CommandDevices {
    pub fn create(uid: u32, gid: u32) -> io::Result<Self> {
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
        for (name, minor) in [
            ("null", 3),
            ("zero", 5),
            ("full", 7),
            ("random", 8),
            ("urandom", 9),
        ] {
            fs::mknodat(
                &root,
                name,
                FileType::CharacterDevice,
                Mode::from_raw_mode(0o666),
                fs::makedev(1, minor),
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
        for (name, target) in [
            ("fd", "/proc/self/fd"),
            ("stdin", "/proc/self/fd/0"),
            ("stdout", "/proc/self/fd/1"),
            ("stderr", "/proc/self/fd/2"),
        ] {
            fs::symlinkat(target, &root, name)?;
        }
        Ok(Self { root })
    }

    /// Transfer to the trusted setup helper through its reserved stdin slot.
    /// The final image shell replaces this slot with trusted null input.
    pub fn into_input(self) -> io::Result<Stdio> {
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
        Ok(Stdio::from(std::os::fd::OwnedFd::from(receiver)))
    }

    /// Receive only on the trusted helper launch channel, never a recipe stream.
    /// Descriptor duplication is close-on-exec. The sender is responsible for
    /// supplying the private tree created above; a tmpfs check is not identity
    /// authentication for an untrusted sender.
    pub fn from_input() -> io::Result<Self> {
        let socket = io::stdin().as_fd().try_clone_to_owned()?;
        let mut bytes = [0u8; 4];
        let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut storage);
        let received = recvmsg(
            &socket,
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
        if !root.metadata()?.is_dir() || fs::fstatfs(&root)?.f_type != 0x0102_1994 {
            return Err(io::Error::other(
                "private device input must be a tmpfs directory",
            ));
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
        Ok(())
    }
}
