// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    fs::File,
    io::{self, BufRead, BufReader},
    os::unix::fs::MetadataExt,
};

/// Pins the namespace in which trusted control resources were acquired.
/// Cleanup must not operate through paths from a different mount namespace.
#[derive(Debug)]
pub(crate) struct ControlMountNamespace {
    mount: File,
}
impl ControlMountNamespace {
    /// A descriptor may refer to a detached or foreign-namespace mount even
    /// while this thread is in the captured namespace. Require its pinned mount ID
    /// to be present in this namespace's actual kernel mount table.
    pub(crate) fn verify_attached(&self, file: &File) -> io::Result<()> {
        self.verify_current()?;
        let mount = rustix::fs::statx(
            file,
            "",
            rustix::fs::AtFlags::EMPTY_PATH,
            rustix::fs::StatxFlags::MNT_ID,
        )?;
        if mount.stx_mask & rustix::fs::StatxFlags::MNT_ID.bits() == 0 || mount.stx_mnt_id == 0 {
            return Err(io::Error::other(
                "kernel did not report the control mount ID",
            ));
        }
        let table = File::open("/proc/thread-self/mountinfo")?;
        if rustix::fs::fstatfs(&table)?.f_type != 0x9fa0 {
            return Err(io::Error::other(
                "control mount verification requires procfs",
            ));
        }
        for line in BufReader::new(table).lines() {
            let line = line?;
            let id = line
                .split_whitespace()
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .ok_or_else(|| io::Error::other("invalid kernel mount ID"))?;
            if id == mount.stx_mnt_id {
                return Ok(());
            }
        }
        Err(io::Error::other(
            "control mount is not attached in the captured namespace",
        ))
    }
    pub(crate) fn capture() -> io::Result<Self> {
        let scope = Self {
            mount: File::open("/proc/thread-self/ns/mnt")?,
        };
        scope.verify_current()?;
        Ok(scope)
    }
    pub(crate) fn verify_current(&self) -> io::Result<()> {
        Self::same_namespace(&self.mount, &File::open("/proc/thread-self/ns/mnt")?)
    }
    fn same_namespace(left: &File, right: &File) -> io::Result<()> {
        let a = left.metadata()?;
        let b = right.metadata()?;
        if rustix::fs::fstatfs(left)?.f_type as u64 != 0x6e73_6673
            || rustix::fs::fstatfs(right)?.f_type as u64 != 0x6e73_6673
            || (a.dev(), a.ino()) != (b.dev(), b.ino())
        {
            return Err(io::Error::other(
                "control mount namespace changed since acquisition",
            ));
        }
        Ok(())
    }
}
