// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    anchor::{CreateFailure as PinFailure, NamespacePin, RecoveryDirectory},
    process::{Identified, LaunchFailure, Process, Supervision, WaitFailure},
};
use nix::sched::{CloneFlags, unshare};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::unix::fs::MetadataExt,
    process::{Command, ExitStatus},
    time::Instant,
};

/// Contiguous mapping from namespace ID zero to nonzero parent IDs. The caller
/// allocates disjoint ranges; neither zero nor the invalid all-ones ID is mapped.
#[derive(Debug, Clone, Copy)]
pub struct IdRange {
    first: u32,
    count: u32,
}
impl IdRange {
    pub fn new(first: u32, count: u32) -> io::Result<Self> {
        if first == 0 || count == 0 || first.checked_add(count).is_none() {
            return Err(io::Error::other("invalid mapped ID range"));
        }
        Ok(Self { first, count })
    }
    fn line(self) -> String {
        format!("0 {} {}\n", self.first, self.count)
    }
}

/// A mapped namespace retained after its short-lived creator has been reaped.
/// No raw namespace handle escapes. Pin it before any labeling authorization
/// needs to survive supervisor death; a descriptor alone does not provide that.
#[derive(Debug)]
pub struct MappedUserNamespace {
    namespace: File,
}
#[derive(Debug, thiserror::Error)]
pub enum UserNamespaceFailure {
    #[error("namespace creator launch: {0}")]
    Launch(#[from] LaunchFailure),
    #[error("namespace mapping: {source}")]
    Mapping {
        source: io::Error,
        process: Process<Identified>,
    },
    #[error("namespace creator reap: {0}")]
    Reap(#[from] WaitFailure<Identified>),
    #[error("namespace creator failed: {0}")]
    Exited(ExitStatus),
}
impl MappedUserNamespace {
    /// Launch only the trusted helper below, in the supervisor's matching procfs
    /// PID view with exclusive child wait ownership and normal SIGCHLD semantics.
    /// Mapping errors retain the child for cancellation/reaping. No image code
    /// runs in this helper, and no policy has yet been enabled for its namespace.
    pub fn create(
        setup: Command,
        uid: IdRange,
        gid: IdRange,
        deadline: Instant,
    ) -> Result<Self, UserNamespaceFailure> {
        let (process, channel) = Process::launch_supervised(setup, deadline)?;
        let mapped = Self::configure(&process, uid, gid);
        // Channel closure explicitly releases the creator; it never polls a
        // marker file or sleeps waiting for mappings. Peer death also releases it.
        drop(channel);
        let mapped = match mapped {
            Ok(mapped) => mapped,
            Err(source) => return Err(UserNamespaceFailure::Mapping { source, process }),
        };
        let status = process.wait_until(deadline)?.status();
        if !status.success() {
            return Err(UserNamespaceFailure::Exited(status));
        }
        Ok(mapped)
    }

    fn configure(process: &Process<Identified>, uid: IdRange, gid: IdRange) -> io::Result<Self> {
        // The owned child cannot have its PID reused before this owner reaps it.
        // Its trusted protocol announces readiness only after unshare(NEWUSER).
        let task = process.proc_directory()?;
        let namespace: File = rustix::fs::openat(
            &task,
            "ns/user",
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?
        .into();
        let ours = fs::metadata("/proc/self/ns/user")?;
        let selected = namespace.metadata()?;
        if (ours.dev(), ours.ino()) == (selected.dev(), selected.ino()) {
            return Err(io::Error::other(
                "creator did not create a new user namespace",
            ));
        }
        for (name, range) in [("uid_map", uid), ("gid_map", gid)] {
            let mut map: File = rustix::fs::openat(
                &task,
                name,
                rustix::fs::OFlags::RDWR
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NOFOLLOW,
                rustix::fs::Mode::empty(),
            )?
            .into();
            let mut existing = String::new();
            (&mut map).take(4096).read_to_string(&mut existing)?;
            if !existing.is_empty() {
                return Err(io::Error::other("creator namespace was already mapped"));
            }
            // procfs requires a single write at offset zero, never a partial
            // write_all retry or an update to an already populated mapping.
            use std::io::{Seek, SeekFrom};
            map.seek(SeekFrom::Start(0))?;
            let line = range.line();
            if map.write(line.as_bytes())? != line.len() {
                return Err(io::Error::other("short namespace mapping write"));
            }
        }
        Ok(Self { namespace })
    }

    pub fn pin(self, directory: &RecoveryDirectory) -> Result<NamespacePin, PinFailure> {
        NamespacePin::create(directory, self.namespace)
    }

    /// Dedicated trusted creator entrypoint, before spawning threads. It only
    /// creates a user namespace, exchanges stable process identities, and waits
    /// for the parent to close its channel after taking namespace ownership.
    pub fn serve(deadline: Instant) -> io::Result<()> {
        if fs::read_dir("/proc/self/task")?
            .collect::<io::Result<Vec<_>>>()?
            .len()
            != 1
        {
            return Err(io::Error::other("namespace creation requires one thread"));
        }
        let channel = Supervision::from_stdin()?;
        unshare(CloneFlags::CLONE_NEWUSER)?;
        channel
            .exchange_until(deadline)?
            .wait_for_loss_until(deadline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_excludes_host_root_invalid_ids_and_overflow() {
        for (first, count) in [(0, 1), (1, 0), (u32::MAX, 1), (u32::MAX - 1, 2)] {
            assert!(IdRange::new(first, count).is_err());
        }
        assert_eq!(
            IdRange::new(100000, 65536).unwrap().line(),
            "0 100000 65536\n"
        );
        assert!(IdRange::new(u32::MAX - 1, 1).is_ok());
    }
}
