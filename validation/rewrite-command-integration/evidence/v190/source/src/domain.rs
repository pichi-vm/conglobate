// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Fresh private cgroup-v2 domains. Registration and reaping are separate gates.
mod drain;
mod events;
pub use drain::{DescendantDrain, DescendantDrainFailure, DrainedFinishFailure, DrainedHelper};
mod helper;
pub use helper::{
    AdmissionFailure, ContainedHelper, HelperCancelFailure, HelperCancellation,
    HelperFinishFailure, HelperLaunchFailure, HelperReapFailure, HelperReaping, ReapedHelper,
};
mod recorded;
pub use recorded::{
    DomainCancellation, DomainCreationCleanupFailure, DomainCreationRollback, DomainRecordCleanup,
    RecordCleanupFailure, RecordDomainFailure, RecordedCancelFailure, RecordedDomain,
};
mod registration;
pub use registration::DomainRegistration;
mod record;
pub use record::DomainRecord;

use crate::anchor::{RecoveryDirectory, RecoveryLease};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    time::Instant,
};

/// A fresh cgroup owned exclusively by trusted control code. It must never
/// contain the supervisor, and commands must not receive its descriptor
/// or control paths. Drop closes descriptors but leaves the kernel domain for
/// recovery; it is not a cancellation or quiescence receipt.
#[derive(Debug)]
pub struct CommandDomain {
    root: File,
    target: DomainTarget,
}
#[derive(Debug)]
pub struct DomainTarget {
    parent: File,
    name: std::ffi::OsString,
    lease: RecoveryLease,
}
#[derive(Debug, thiserror::Error)]
pub enum DomainCreateFailure {
    #[error("domain before directory creation: {0}")]
    Before(io::Error),
    #[error("domain setup: {source}")]
    Created {
        source: io::Error,
        target: DomainTarget,
    },
}
#[derive(Debug, thiserror::Error)]
#[error("domain cancellation: {source}")]
pub struct DomainFailure {
    source: io::Error,
    domain: CommandDomain,
}
impl DomainFailure {
    pub fn into_parts(self) -> (io::Error, CommandDomain) {
        (self.source, self.domain)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("domain target cleanup: {source}")]
pub struct DomainTargetFailure {
    source: io::Error,
    target: DomainTarget,
}
impl DomainTargetFailure {
    pub fn into_parts(self) -> (io::Error, DomainTarget) {
        (self.source, self.target)
    }
}
impl DomainTarget {
    fn open_parent(parent: &Path) -> io::Result<File> {
        let parent = File::from(rustix::fs::open(
            parent,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?);
        let metadata = parent.metadata()?;
        if metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || rustix::fs::fstatfs(&parent)?.f_type as u64 != 0x6367_7270
        {
            return Err(io::Error::other(
                "domain parent must be protected root-owned cgroup2",
            ));
        }
        Ok(parent)
    }

    fn path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.parent.as_raw_fd())).join(&self.name)
    }
    /// Only for a fresh setup failure before registration. Errors retain the
    /// target. Never recursively delete cgroup control files or child domains.
    pub fn remove(self) -> Result<(), DomainTargetFailure> {
        let result = self
            .lease
            .verify_current()
            .and_then(|()| fs::remove_dir(self.path()));
        result.map_err(|source| DomainTargetFailure {
            source,
            target: self,
        })
    }
}
impl CommandDomain {
    fn registration_file(&self) -> io::Result<File> {
        self.target.lease.verify_current()?;
        Ok(File::from(rustix::fs::openat(
            &self.root,
            "cgroup.procs",
            rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?))
    }

    /// Create only; no API accepts an arbitrary existing cgroup to kill.
    /// Caller must serialize this protected parent and persist ownership before
    /// registration. This primitive neither mounts cgroupfs nor sets limits.
    pub fn create(
        recovery: &RecoveryDirectory,
        parent: &Path,
    ) -> Result<Self, DomainCreateFailure> {
        let lease = recovery.retain().map_err(DomainCreateFailure::Before)?;
        let parent = DomainTarget::open_parent(parent).map_err(DomainCreateFailure::Before)?;
        let directory = tempfile::Builder::new()
            .prefix("conglobate-command-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))
            .map_err(DomainCreateFailure::Before)?;
        // cgroupfs has kernel-owned entries: never use recursive TempDir cleanup.
        let path = directory.keep();
        let target = DomainTarget {
            parent,
            name: path.file_name().expect("generated name").to_owned(),
            lease,
        };
        let root = match File::open(target.path()) {
            Ok(root) => root,
            Err(source) => return Err(DomainCreateFailure::Created { source, target }),
        };
        let domain = Self { root, target };
        if let Err(source) = domain.validate_new() {
            return Err(DomainCreateFailure::Created {
                source,
                target: domain.target,
            });
        }
        Ok(domain)
    }
    fn entry(&self, name: &str) -> io::Result<File> {
        Ok(File::from(rustix::fs::openat(
            &self.root,
            name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?))
    }
    fn text(&self, name: &str) -> io::Result<String> {
        let mut text = String::new();
        self.entry(name)?.take(4097).read_to_string(&mut text)?;
        if text.len() > 4096 {
            return Err(io::Error::other("oversized cgroup control value"));
        }
        Ok(text)
    }
    fn validate_new(&self) -> io::Result<()> {
        if self.text("cgroup.type")?.trim() != "domain" || self.populated()? {
            return Err(io::Error::other(
                "fresh command cgroup is not an empty domain",
            ));
        }
        Ok(())
    }
    fn populated(&self) -> io::Result<bool> {
        DomainEvents::try_from(self.text("cgroup.events")?.as_str()).map(|events| events.populated)
    }
    /// Cancel the complete subtree, then remove this directory after observing
    /// populated=0. Call only after stopping admission. Success is not proof of
    /// reaping, policy removal, filesystem cleanup or command success.
    pub fn cancel_until(self, deadline: Instant) -> Result<(), DomainFailure> {
        match self.cancel(deadline) {
            Ok(()) => Ok(()),
            Err(source) => Err(DomainFailure {
                source,
                domain: self,
            }),
        }
    }
    fn cancel(&self, deadline: Instant) -> io::Result<()> {
        self.target.lease.verify_current()?;
        // Cancellation is requested even if the teardown deadline has expired.
        let mut kill = File::from(rustix::fs::openat(
            &self.root,
            "cgroup.kill",
            rustix::fs::OFlags::WRONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?);
        kill.write_all(b"1")?;
        events::DomainWatch::new(self.entry("cgroup.events")?).wait_empty_until(deadline)?;
        fs::remove_dir(self.target.path())
    }
}
#[derive(Debug)]
struct DomainEvents {
    populated: bool,
}
impl TryFrom<&str> for DomainEvents {
    type Error = io::Error;
    fn try_from(text: &str) -> io::Result<Self> {
        let mut populated = None;
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            if fields.next() == Some("populated") {
                let value = match fields.next() {
                    Some("0") => false,
                    Some("1") => true,
                    _ => return Err(io::Error::other("invalid cgroup populated value")),
                };
                if fields.next().is_some() || populated.replace(value).is_some() {
                    return Err(io::Error::other("ambiguous cgroup populated value"));
                }
            }
        }
        populated
            .map(|populated| Self { populated })
            .ok_or_else(|| io::Error::other("missing cgroup populated value"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn event_status_requires_exact_unambiguous_population() {
        assert!(
            !DomainEvents::try_from("populated 0\nfrozen 0\n")
                .unwrap()
                .populated
        );
        assert!(
            DomainEvents::try_from("frozen 0\npopulated 1\n")
                .unwrap()
                .populated
        );
        for input in [
            "",
            "frozen 0",
            "populated 2",
            "populated 0 1",
            "populated 0\npopulated 0",
        ] {
            assert!(DomainEvents::try_from(input).is_err());
        }
    }
    #[test]
    fn ordinary_directory_is_rejected_before_creation() {
        let root = tempfile::tempdir().unwrap();
        assert!(DomainTarget::open_parent(root.path()).is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 0);
    }
}
