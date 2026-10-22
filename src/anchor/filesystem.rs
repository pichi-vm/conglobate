// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{DirectoryLease, RecoveryDirectory};
use std::{
    fs::{self, File, Metadata},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::PathBuf,
    sync::Arc,
};

/// Kernel-held filesystem reference in the deployment init's mount namespace.
/// Drop deliberately leaves the bind mount intact until VM disposal. Explicit
/// release is only valid after all policy referencing its superblock is gone.
/// This pins a filesystem identity; it does not establish its policy owner,
/// command admission, exclusive write access, or successful image finalization.
#[derive(Debug)]
pub struct FilesystemPin {
    target: FilesystemPinTarget,
    device: u64,
    inode: u64,
}

/// An empty, unmounted pin directory. Cleanup never recursively traverses it.
#[derive(Debug)]
pub struct FilesystemPinTarget {
    parent: Arc<DirectoryLease>,
    name: std::ffi::OsString,
}

#[derive(Debug, thiserror::Error)]
pub enum FilesystemPinFailure {
    #[error("filesystem pin before mount: {0}")]
    Before(#[from] io::Error),
    #[error("filesystem pin mount: {source}")]
    Mount {
        source: io::Error,
        target: FilesystemPinTarget,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum FilesystemPinReleaseFailure {
    #[error("filesystem pin remains mounted: {source}")]
    Mounted {
        source: io::Error,
        pin: FilesystemPin,
    },
    #[error("filesystem pin target cleanup: {source}")]
    Target {
        source: io::Error,
        target: FilesystemPinTarget,
    },
}

impl FilesystemPin {
    /// Bind only the selected directory view, excluding its submounts. The
    /// trusted caller must prevent out-of-protocol mutation of the pin parent.
    pub fn create(parent: &RecoveryDirectory, source: File) -> Result<Self, FilesystemPinFailure> {
        parent.lease.namespace.verify_current()?;
        let metadata = source.metadata()?;
        if !metadata.is_dir() {
            return Err(io::Error::other("filesystem pin source must be a directory").into());
        }
        let directory = tempfile::Builder::new()
            .prefix("filesystem-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(format!(
                "/proc/self/fd/{}",
                parent.lease.directory.as_raw_fd()
            ))?;
        // Disarm recursive cleanup before any filesystem can be mounted here.
        let path = directory.keep();
        let target = FilesystemPinTarget {
            parent: Arc::clone(&parent.lease),
            name: path
                .file_name()
                .expect("generated directory name")
                .to_owned(),
        };
        if let Err(source) = rustix::mount::mount_bind(
            format!("/proc/self/fd/{}", source.as_raw_fd()),
            target.path(),
        ) {
            return Err(FilesystemPinFailure::Mount {
                source: source.into(),
                target,
            });
        }
        // No fallible operation after mount: ownership transfers intact.
        Ok(Self {
            target,
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    pub fn metadata(&self) -> io::Result<Metadata> {
        self.target.parent.namespace.verify_current()?;
        let file = File::from(rustix::fs::openat(
            &self.target.parent.directory,
            &self.target.name,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        )?);
        let metadata = file.metadata()?;
        if (metadata.dev(), metadata.ino()) != (self.device, self.inode) {
            return Err(io::Error::other("filesystem pin identity mismatch"));
        }
        Ok(metadata)
    }

    pub fn release(self) -> Result<(), FilesystemPinReleaseFailure> {
        if let Err(source) = self.metadata() {
            return Err(FilesystemPinReleaseFailure::Mounted { source, pin: self });
        }
        if let Err(source) =
            rustix::mount::unmount(self.target.path(), rustix::mount::UnmountFlags::NOFOLLOW)
        {
            return Err(FilesystemPinReleaseFailure::Mounted {
                source: source.into(),
                pin: self,
            });
        }
        self.target.cleanup()
    }
}

impl FilesystemPinTarget {
    fn path(&self) -> PathBuf {
        PathBuf::from(format!(
            "/proc/self/fd/{}",
            self.parent.directory.as_raw_fd()
        ))
        .join(&self.name)
    }
    pub fn cleanup(self) -> Result<(), FilesystemPinReleaseFailure> {
        if let Err(source) = fs::remove_dir(self.path()) {
            return Err(FilesystemPinReleaseFailure::Target {
                source,
                target: self,
            });
        }
        Ok(())
    }
}
impl Drop for FilesystemPinTarget {
    fn drop(&mut self) {
        // rmdir cannot remove an attached mount or traverse image contents.
        // A live FilesystemPin deliberately leaves that mount for VM disposal.
        if let Err(error) = fs::remove_dir(self.path())
            && error.kind() != io::ErrorKind::NotFound
            && error.raw_os_error() != Some(rustix::io::Errno::BUSY.raw_os_error())
        {
            eprintln!("filesystem pin target retained: {error}");
        }
    }
}
