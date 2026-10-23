// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    process::{Pid, PidfdFlags, getpid, pidfd_open},
};
use std::{
    fs::{self, File},
    io::{self, BufRead, BufReader},
    os::unix::fs::MetadataExt,
};

/// Observation of the deployment init's namespace, not a host-global identity
/// proof. The deployment must keep init alive and in this namespace independently
/// of builds. No arbitrary PID, signaling or namespace-entry authority escapes.
#[derive(Debug)]
pub(crate) struct InitMountNamespace {
    init: OwnedFd,
    mount: File,
}
impl InitMountNamespace {
    /// A descriptor may refer to a detached or foreign-namespace mount even
    /// while this thread is in init's namespace. Require its pinned mount ID
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
            "control mount is not attached in the deployment init namespace",
        ))
    }
    pub(crate) fn capture() -> io::Result<Self> {
        if getpid() == Pid::INIT
            || fs::read_link("/proc/self")?.to_str() != Some(&getpid().as_raw_nonzero().to_string())
        {
            return Err(io::Error::other(
                "recovery requires an independent init and matching procfs PID view",
            ));
        }
        let init = pidfd_open(Pid::INIT, PidfdFlags::empty())?;
        let scope = Self {
            init,
            mount: File::open("/proc/1/ns/mnt")?,
        };
        scope.verify_current()?;
        Ok(scope)
    }
    pub(crate) fn verify_current(&self) -> io::Result<()> {
        if fs::read_link("/proc/self")?.to_str() != Some(&getpid().as_raw_nonzero().to_string()) {
            return Err(io::Error::other("recovery procfs PID view changed"));
        }
        Self::same_namespace(
            &File::open("/proc/1/ns/pid")?,
            &File::open("/proc/thread-self/ns/pid")?,
        )?;

        Self::same_namespace(&self.mount, &File::open("/proc/1/ns/mnt")?)?;
        Self::same_namespace(&self.mount, &File::open("/proc/thread-self/ns/mnt")?)?;
        let mut observed = [PollFd::new(&self.init, PollFlags::IN)];
        poll(
            &mut observed,
            Some(&Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            }),
        )?;
        if !observed[0].revents().is_empty() {
            return Err(io::Error::other("recovery init is no longer live"));
        }
        Ok(())
    }
    fn same_namespace(left: &File, right: &File) -> io::Result<()> {
        let a = left.metadata()?;
        let b = right.metadata()?;
        if rustix::fs::fstatfs(left)?.f_type as u64 != 0x6e73_6673
            || rustix::fs::fstatfs(right)?.f_type as u64 != 0x6e73_6673
            || (a.dev(), a.ino()) != (b.dev(), b.ino())
        {
            return Err(io::Error::other(
                "recovery must use the deployment init's mount/PID namespace",
            ));
        }
        Ok(())
    }
}
