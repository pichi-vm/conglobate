// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use rustix::{
    fs::{self, AtFlags, FileType, Mode},
    mount::{self, FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags},
    process::{Gid, Uid},
};
use std::{
    fs::File,
    io::{self},
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
