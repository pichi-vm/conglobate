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
    os::unix::process::ExitStatusExt,
    process::{Child, Command, ExitStatus},
};

/// Resident init inside the command's restricted namespaces and image root.
/// It retains only status-reporting authority beyond its standard descriptors.
/// No backing/control handles reach image exec. Errors are exit-only.
#[derive(Debug)]
pub(super) struct NamespaceInit {
    shell: Child,
    reporter: Option<super::CompletionEndpoint>,
}
impl NamespaceInit {
    pub(super) fn spawn(
        mut command: Command,
        reporter: Option<super::CompletionEndpoint>,
    ) -> io::Result<Self> {
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
            reporter,
        })
    }

    pub(super) fn execute(self) -> io::Result<Infallible> {
        let shell = Pid::from_raw(self.shell.id() as i32);
        loop {
            // This owner is the sole waiter. __WALL includes adopted clone
            // children regardless of their exit-signal selection. No polling
            // delay or signal handler is needed: waitpid blocks for an event.
            let status = match waitpid(None, Some(WaitPidFlag::__WALL)) {
                Ok(WaitStatus::Exited(pid, code)) if pid == shell => {
                    Some(ExitStatus::from_raw(code << 8))
                }
                Ok(WaitStatus::Signaled(pid, signal, core_dumped)) if pid == shell => Some(
                    ExitStatus::from_raw(signal as i32 | if core_dumped { 0x80 } else { 0 }),
                ),
                Ok(_) | Err(Errno::EINTR) => None,
                Err(error) => return Err(error.into()),
            };
            if let Some(status) = status {
                if let Some(reporter) = self.reporter {
                    reporter.report(status)?;
                }
                let code = status
                    .code()
                    .unwrap_or_else(|| 128 + status.signal().unwrap_or(1));
                // Exiting PID 1 kills remaining namespace members in the
                // kernel. The outer step must still cancel its cgroup, reap
                // helpers and establish quiescence before releasing storage.
                std::process::exit(code);
            }
        }
    }
}
