// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Protected runtime directory lease in the acquisition mount namespace.
use std::{fs::File, io, os::unix::fs::MetadataExt, path::Path, sync::Arc};

mod namespace;
pub(crate) use namespace::ControlMountNamespace;

/// Exclusive cooperative lease on a root-only tmpfs recovery directory.
/// The descriptor stays locked while any recovery owner derived from this
/// owner lives. This does not prevent privileged callers bypassing the protocol,
/// namespace disappearance, or replacement of the directory by its parent owner.
#[derive(Debug)]
pub(crate) struct RecoveryDirectory {
    lease: Arc<DirectoryLease>,
}
/// Internal shared lock ownership; no descriptor or early unlock escapes.
#[derive(Debug, Clone)]
pub(crate) struct RecoveryLease(Arc<DirectoryLease>);
impl RecoveryLease {
    pub(crate) fn verify_current(&self) -> io::Result<()> {
        self.0.namespace.verify_current()
    }
}
impl RecoveryDirectory {
    pub(crate) fn verify_current(&self) -> io::Result<()> {
        self.lease.namespace.verify_current()
    }
    pub(crate) fn retain(&self) -> io::Result<RecoveryLease> {
        self.lease.namespace.verify_current()?;
        Ok(RecoveryLease(Arc::clone(&self.lease)))
    }

    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        let namespace = ControlMountNamespace::capture()?;
        let file = DirectoryLease::open_directory(path)?;
        namespace.verify_attached(&file)?;
        rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
        Ok(Self {
            lease: Arc::new(DirectoryLease {
                _directory: file,
                namespace,
            }),
        })
    }
}

#[derive(Debug)]
struct DirectoryLease {
    _directory: File,
    namespace: ControlMountNamespace,
}

impl DirectoryLease {
    fn open_directory(path: &Path) -> io::Result<File> {
        let file = File::from(rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?);
        let metadata = file.metadata()?;
        if !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.mode() & 0o777 != 0o700
            || rustix::fs::fstatfs(&file)?.f_type as u64 != 0x0102_1994
        {
            return Err(io::Error::other(
                "recovery directory must be a root-owned mode-0700 tmpfs directory",
            ));
        }
        Ok(file)
    }
}
