// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::process::{Identified, LaunchFailure, Process, Supervision, WaitFailure};
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
    pub(crate) fn first(self) -> u32 {
        self.first
    }
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

/// A mapped namespace owned by a descriptor after its creator has been reaped.
/// Retain this owner through command setup and descendant quiescence. Mounts
/// and running tasks also retain their kernel namespace references.
#[derive(Debug)]
pub struct UserNamespace<S: UserNamespaceState> {
    resources: S::Resources,
}
mod private {
    pub trait Sealed {}
}
pub trait UserNamespaceState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub struct Mapped;
impl private::Sealed for Mapped {}
impl UserNamespaceState for Mapped {
    type Resources = File;
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
impl UserNamespace<Mapped> {
    /// Launch only the trusted helper below, in the supervisor's matching procfs
    /// PID view with exclusive child wait ownership and normal SIGCHLD semantics.
    /// Mapping errors retain the child for cancellation/reaping. No image code
    /// runs in this helper; the supervisor supplies both ID ranges.
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
        Ok(Self {
            resources: namespace,
        })
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

impl UserNamespace<Mapped> {
    pub(crate) fn descriptor(&self) -> io::Result<File> {
        self.resources.try_clone()
    }
    pub fn idmap<'ns>(
        &self,
        mount: crate::filesystem::Mount<'ns, crate::filesystem::Detached>,
    ) -> Result<crate::filesystem::Mount<'ns, crate::filesystem::Idmapped>, NamespaceViewFailure<'ns>>
    {
        match self.resources.try_clone() {
            Ok(namespace) => mount
                .idmap(namespace)
                .map_err(NamespaceViewFailure::Mapping),
            Err(source) => Err(NamespaceViewFailure::Descriptor { source, mount }),
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum NamespaceViewFailure<'ns> {
    #[error("duplicate mapped namespace descriptor: {source}")]
    Descriptor {
        source: io::Error,
        mount: crate::filesystem::Mount<'ns, crate::filesystem::Detached>,
    },
    #[error("apply namespace mapping: {0}")]
    Mapping(crate::filesystem::IdmapFailure<'ns>),
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
