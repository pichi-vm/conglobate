// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Kernel-held namespace pins in the deployment init's mount namespace.
//! The deployment must keep init alive independently of builds. Runtime checks
//! reject a different namespace; they do not prove host-global identity or
//! authorize policy activation.
use std::{
    ffi::OsStr,
    fs::{self, File},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    sync::Arc,
};

mod namespace;
mod record;
use namespace::InitMountNamespace;

/// Exclusive cooperative lease on a root-only tmpfs recovery directory.
/// The descriptor stays locked while any pin/recovery target derived from this
/// owner lives. This does not prevent privileged callers bypassing the protocol,
/// namespace disappearance, or replacement of the directory by its parent owner.
#[derive(Debug)]
pub struct RecoveryDirectory {
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
    pub(crate) fn retain(&self) -> io::Result<RecoveryLease> {
        self.lease.namespace.verify_current()?;
        Ok(RecoveryLease(Arc::clone(&self.lease)))
    }

    pub fn open(path: &Path) -> io::Result<Self> {
        let namespace = InitMountNamespace::capture()?;
        let file = Backing::parent(path)?;
        rustix::fs::flock(&file, rustix::fs::FlockOperation::NonBlockingLockExclusive)?;
        Ok(Self {
            lease: Arc::new(DirectoryLease {
                directory: file,
                namespace,
            }),
        })
    }
}

#[derive(Debug)]
struct DirectoryLease {
    directory: File,
    namespace: InitMountNamespace,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceRecord {
    name: std::ffi::OsString,
    device: u64,
    inode: u64,
}
impl NamespaceRecord {
    pub fn name(&self) -> &OsStr {
        &self.name
    }
    pub fn inode(&self) -> u64 {
        self.inode
    }
}
#[derive(Debug)]
struct Backing {
    parent: Arc<DirectoryLease>,
    record: NamespaceRecord,
}
impl Backing {
    fn parent(path: &Path) -> io::Result<File> {
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
                "namespace pin parent must be a root-owned mode-0700 tmpfs directory",
            ));
        }
        Ok(file)
    }
    fn path(&self) -> PathBuf {
        PathBuf::from(format!(
            "/proc/self/fd/{}",
            self.parent.directory.as_raw_fd()
        ))
        .join(&self.record.name)
    }
    fn verify(&self) -> io::Result<()> {
        self.parent.namespace.verify_current()?;
        let file = File::from(rustix::fs::openat(
            &self.parent.directory,
            &self.record.name,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?);
        let metadata = file.metadata()?;
        if rustix::fs::fstatfs(&file)?.f_type as u64 != 0x6e73_6673
            || (metadata.dev(), metadata.ino()) != (self.record.device, self.record.inode)
        {
            return Err(io::Error::other("namespace pin identity mismatch"));
        }
        Ok(())
    }
}

/// A namespace retained by a bind mount, not just an open descriptor. Dropping
/// this owner intentionally leaves the mount for recovery. Preserve its record
/// in the build lease before admission. Explicit release requires that every
/// policy referencing this identity is already removed.
#[derive(Debug)]
pub struct NamespacePin {
    backing: Backing,
}

/// An unmounted pin target awaiting unlink. Drop only unlinks this known file.
#[derive(Debug)]
pub struct PinTarget {
    backing: Option<Backing>,
}
#[derive(Debug, thiserror::Error)]
pub enum CreateFailure {
    #[error("namespace pin before mount: {0}")]
    Before(#[from] io::Error),
    #[error("namespace pin mount failed: {source}")]
    Mount {
        source: io::Error,
        target: PinTarget,
    },
}
#[derive(Debug, thiserror::Error)]
pub enum ReleaseFailure {
    #[error("namespace pin remains mounted: {source}")]
    Mounted {
        source: io::Error,
        pin: NamespacePin,
    },
    #[error("namespace pin unmounted, target cleanup failed: {source}")]
    Target {
        source: io::Error,
        target: PinTarget,
    },
}
impl NamespacePin {
    pub fn create(parent: &RecoveryDirectory, namespace: File) -> Result<Self, CreateFailure> {
        parent.lease.namespace.verify_current()?;
        if rustix::fs::fstatfs(&namespace)
            .map_err(io::Error::from)?
            .f_type as u64
            != 0x6e73_6673
        {
            return Err(io::Error::other("source is not a namespace descriptor").into());
        }
        let metadata = namespace.metadata()?;
        let parent = Arc::clone(&parent.lease);
        let temporary = tempfile::Builder::new()
            .prefix("namespace-")
            .permissions(fs::Permissions::from_mode(0o600))
            .tempfile_in(format!("/proc/self/fd/{}", parent.directory.as_raw_fd()))?;
        let name = temporary
            .path()
            .file_name()
            .expect("generated filename")
            .to_owned();
        let (file, _) = temporary.keep().map_err(|error| error.error)?;
        drop(file);
        let mut target = PinTarget {
            backing: Some(Backing {
                parent,
                record: NamespaceRecord {
                    name,
                    device: metadata.dev(),
                    inode: metadata.ino(),
                },
            }),
        };
        if let Err(source) = rustix::mount::mount_bind(
            format!("/proc/self/fd/{}", namespace.as_raw_fd()),
            target.backing.as_ref().expect("owned target").path(),
        ) {
            return Err(CreateFailure::Mount {
                source: source.into(),
                target,
            });
        }
        // Only infallible ownership transfer follows successful mount creation.
        Ok(Self {
            backing: target.backing.take().expect("owned target"),
        })
    }
    pub fn record(&self) -> &NamespaceRecord {
        &self.backing.record
    }
    pub fn recover(parent: &RecoveryDirectory, record: NamespaceRecord) -> io::Result<Self> {
        let backing = Backing {
            parent: Arc::clone(&parent.lease),
            record,
        };
        backing.verify()?;
        Ok(Self { backing })
    }
    pub fn release(self) -> Result<(), ReleaseFailure> {
        if let Err(source) = self.backing.verify() {
            return Err(ReleaseFailure::Mounted { source, pin: self });
        }
        if let Err(source) =
            rustix::mount::unmount(self.backing.path(), rustix::mount::UnmountFlags::NOFOLLOW)
        {
            return Err(ReleaseFailure::Mounted {
                source: source.into(),
                pin: self,
            });
        }
        PinTarget {
            backing: Some(self.backing),
        }
        .cleanup()
    }
}
impl PinTarget {
    pub fn cleanup(mut self) -> Result<(), ReleaseFailure> {
        if let Some(backing) = &self.backing
            && let Err(source) = fs::remove_file(backing.path())
        {
            return Err(ReleaseFailure::Target {
                source,
                target: self,
            });
        }
        self.backing.take();
        Ok(())
    }
}
impl Drop for PinTarget {
    fn drop(&mut self) {
        if let Some(backing) = &self.backing
            && let Err(error) = fs::remove_file(backing.path())
        {
            eprintln!("namespace pin target requires cleanup: {error}");
        }
    }
}
