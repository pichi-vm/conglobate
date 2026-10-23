// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use nix::{
    errno::Errno,
    sys::wait::{WaitPidFlag, WaitStatus, waitpid},
    unistd::{Pid, getpid},
};
use std::{
    convert::Infallible,
    fs, io,
    process::{Child, Command},
};

/// Resident init inside the command's restricted namespaces and image root.
/// It has no supervisor authority or outside-root handles. Errors are exit-only.
#[derive(Debug)]
pub(super) struct NamespaceInit {
    shell: Child,
}
impl NamespaceInit {
    pub(super) fn spawn(mut command: Command) -> io::Result<Self> {
        if getpid() != Pid::from_raw(1) {
            return Err(io::Error::other("command init requires PID 1"));
        }
        let map = fs::read_to_string("/proc/self/uid_map")?;
        let root = map.lines().find_map(|line| {
            let values = line
                .split_whitespace()
                .map(str::parse::<u32>)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            match values.as_slice() {
                [0, outside, count] if *outside != 0 && *count != 0 => Some(()),
                _ => None,
            }
        });
        if root.is_none() {
            return Err(io::Error::other(
                "command init requires mapped non-host root",
            ));
        }
        Ok(Self {
            shell: command.spawn()?,
        })
    }

    pub(super) fn execute(self) -> io::Result<Infallible> {
        let shell = Pid::from_raw(self.shell.id() as i32);
        loop {
            // This owner is the sole waiter. __WALL includes adopted clone
            // children regardless of their exit-signal selection. No polling
            // delay or signal handler is needed: waitpid blocks for an event.
            let code = match waitpid(None, Some(WaitPidFlag::__WALL)) {
                Ok(WaitStatus::Exited(pid, code)) if pid == shell => Some(code),
                Ok(WaitStatus::Signaled(pid, signal, _)) if pid == shell => {
                    Some(128 + signal as i32)
                }
                Ok(_) | Err(Errno::EINTR) => None,
                Err(error) => return Err(error.into()),
            };
            if let Some(code) = code {
                // Exiting PID 1 kills remaining namespace members in the
                // kernel. The outer step must still cancel its cgroup, reap
                // helpers and establish quiescence before removing policy.
                std::process::exit(code);
            }
        }
    }
}
