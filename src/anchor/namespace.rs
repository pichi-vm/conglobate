// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use rustix::{
    event::{PollFd, PollFlags, Timespec, poll},
    fd::OwnedFd,
    process::{Pid, PidfdFlags, getpid, pidfd_open},
};
use std::{
    fs::{self, File},
    io,
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
