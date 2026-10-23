// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    anchor::{
        CreateFailure as PinFailure, NamespacePin, NamespaceRecord, PinTarget, RecoveryDirectory,
        ReleaseFailure,
    },
    policy::{FilesystemTarget, LabelScope, ScopeError},
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

/// A mapped namespace retained after its short-lived creator has been reaped.
/// No raw namespace handle escapes. Pin it before any labeling authorization
/// needs to survive supervisor death; a descriptor alone does not provide that.
///
/// ```compile_fail
/// use conglobate::isolation::{Mapped, UserNamespace};
/// fn cannot_describe_pinned_policy(namespace: UserNamespace<Mapped>) {
///     namespace.label_scope([]);
/// }
/// ```
/// ```compile_fail
/// use conglobate::{anchor::RecoveryDirectory, isolation::{PinnedNamespace, UserNamespace}};
/// fn cannot_pin_twice(namespace: UserNamespace<PinnedNamespace>, directory: &RecoveryDirectory) {
///     namespace.pin(directory);
/// }
/// ```
#[derive(Debug)]
pub struct UserNamespace<S: UserNamespaceState> {
    resources: S::Resources,
    uid: IdRange,
}
mod private {
    pub trait Sealed {}
}
pub trait UserNamespaceState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub struct Mapped;
#[derive(Debug)]
pub struct PinnedNamespace;
impl private::Sealed for Mapped {}
impl private::Sealed for PinnedNamespace {}
impl UserNamespaceState for Mapped {
    type Resources = File;
}
impl UserNamespaceState for PinnedNamespace {
    type Resources = NamespacePin;
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
        Ok(Self {
            resources: namespace,
            uid,
        })
    }

    pub fn pin(
        self,
        directory: &RecoveryDirectory,
    ) -> Result<UserNamespace<PinnedNamespace>, PinFailure> {
        let pin = NamespacePin::create(directory, self.resources)?;
        Ok(UserNamespace {
            resources: pin,
            uid: self.uid,
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

/// Cleanup-only failure. A still-mounted pin retains its mapped identity; a
/// successfully unmounted target exposes only the remaining unlink operation.
#[derive(Debug, thiserror::Error)]
pub enum UserNamespaceReleaseFailure {
    #[error("mapped namespace pin remains mounted: {source}")]
    Mounted {
        source: io::Error,
        namespace: UserNamespace<PinnedNamespace>,
    },
    #[error("mapped namespace pin target cleanup: {source}")]
    Target {
        source: io::Error,
        target: PinTarget,
    },
}
impl UserNamespace<PinnedNamespace> {
    pub(crate) fn descriptor(&self) -> io::Result<File> {
        self.resources.namespace()
    }
    /// Apply this pinned mapping without reopening its mountpoint by name.
    /// The mount retains a duplicate namespace descriptor on success; the policy
    /// owner must separately retain the pin until authorization is removed.
    pub fn idmap<'ns>(
        &self,
        mount: crate::filesystem::Mount<'ns, crate::filesystem::Detached>,
    ) -> Result<crate::filesystem::Mount<'ns, crate::filesystem::Idmapped>, NamespaceViewFailure<'ns>>
    {
        match self.resources.namespace() {
            Ok(namespace) => mount
                .idmap(namespace)
                .map_err(NamespaceViewFailure::Mapping),
            Err(source) => Err(NamespaceViewFailure::Descriptor { source, mount }),
        }
    }
    /// Policy description derived from the owned mapping and namespace pin.
    /// This is not activation authority: the enclosing step must retain this
    /// owner and assigned filesystem lifetimes until policy is disabled.
    pub fn label_scope(
        &self,
        filesystems: impl IntoIterator<Item = FilesystemTarget>,
    ) -> Result<LabelScope, ScopeError> {
        LabelScope::new(self.resources.record().inode(), self.uid.first, filesystems)
    }
    pub fn record(&self) -> &NamespaceRecord {
        self.resources.record()
    }
    pub fn release(self) -> Result<(), UserNamespaceReleaseFailure> {
        match self.resources.release() {
            Ok(()) => Ok(()),
            Err(ReleaseFailure::Mounted { source, pin }) => {
                Err(UserNamespaceReleaseFailure::Mounted {
                    source,
                    namespace: Self {
                        resources: pin,
                        uid: self.uid,
                    },
                })
            }
            Err(ReleaseFailure::Target { source, target }) => {
                Err(UserNamespaceReleaseFailure::Target { source, target })
            }
        }
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

/// Both failures retain the detached mount for explicit cleanup or retry.
#[derive(Debug, thiserror::Error)]
pub enum NamespaceViewFailure<'ns> {
    #[error("duplicate pinned namespace descriptor: {source}")]
    Descriptor {
        source: io::Error,
        mount: crate::filesystem::Mount<'ns, crate::filesystem::Detached>,
    },
    #[error("apply pinned namespace mapping: {0}")]
    Mapping(crate::filesystem::IdmapFailure<'ns>),
}
