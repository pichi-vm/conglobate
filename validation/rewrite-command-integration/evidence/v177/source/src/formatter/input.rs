// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::FormatterRuntime;
use rustix::{
    fs::{self, FileType, Mode},
    mount::{self, FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags, OpenTreeFlags},
    net::{
        RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
        SendAncillaryMessage, SendFlags,
    },
};
use std::{
    fs::File,
    io::{self, IoSlice, IoSliceMut},
    mem::MaybeUninit,
    os::{
        fd::{AsFd, OwnedFd},
        unix::net::UnixDatagram,
    },
    process::Stdio,
};

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
        let (sender, receiver) = UnixDatagram::pair()?;
        let descriptors = [self.root.as_fd(), self.devices.as_fd()];
        let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(2))];
        let mut ancillary = SendAncillaryBuffer::new(&mut storage);
        if !ancillary.push(SendAncillaryMessage::ScmRights(&descriptors)) {
            return Err(io::Error::other("formatter descriptor capacity"));
        }
        let sent = rustix::net::sendmsg(
            &sender,
            &[IoSlice::new(&[0x82, 1, 0])],
            &mut ancillary,
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        )?;
        if sent != 3 {
            return Err(io::Error::other("incomplete formatter input"));
        }
        Ok(Stdio::from(OwnedFd::from(receiver)))
    }
    pub(crate) fn from_stdin() -> io::Result<Self> {
        let socket = io::stdin().as_fd().try_clone_to_owned()?;
        let mut bytes = [0u8; 4];
        let mut storage = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(3))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut storage);
        let received = rustix::net::recvmsg(
            &socket,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::CMSG_CLOEXEC | RecvFlags::DONTWAIT,
        )?;
        let mut descriptors = Vec::new();
        for item in ancillary.drain() {
            match item {
                RecvAncillaryMessage::ScmRights(rights) => descriptors.extend(rights),
                _ => return Err(io::Error::other("unexpected formatter ancillary message")),
            }
        }
        if received.bytes != 3
            || bytes[..3] != [0x82, 1, 0]
            || received
                .flags
                .intersects(ReturnFlags::TRUNC | ReturnFlags::CTRUNC)
            || descriptors.len() != 2
        {
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
