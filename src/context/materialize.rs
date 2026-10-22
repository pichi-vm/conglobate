// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CapturedContext, ContextEntry, ContextError, StagedContext};
use crate::path::ContextPath;
use rustix::fs::{Mode, OFlags, ResolveFlags};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::OsStrExt,
            fs::{OpenOptionsExt, PermissionsExt},
        },
    },
    path::Path,
};

/// Private populated checkout on existing tmpfs. Read-only mount enforcement is
/// a separate consuming lifecycle transition, not implied by file permissions.
#[derive(Debug)]
pub struct StagedResources {
    // Cleanup uses /proc/self/fd/<parent>/..., so parent must outlive directory.
    pub(super) directory: tempfile::TempDir,
    pub(super) root: File,
    pub(super) _parent: File,
}
impl CapturedContext {
    pub fn materialize(&self, parent: &Path) -> Result<StagedContext, ContextError> {
        let parent = File::from(
            rustix::fs::open(
                parent,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(io::Error::from)?,
        );
        self.materialize_in(parent)
    }

    /// Internal descriptor authority from the workspace, never a command path.
    /// The returned context owns this parent reference through cleanup.
    pub(crate) fn materialize_in(&self, parent: File) -> Result<StagedContext, ContextError> {
        // Linux tmpfs superblock magic, from linux/magic.h. This only verifies
        // backing type; environment policy governs ordinary tmpfs swap behavior.
        if rustix::fs::fstatfs(&parent)
            .map_err(io::Error::from)?
            .f_type
            != 0x0102_1994
        {
            return Err(ContextError::Invalid("context staging requires tmpfs"));
        }
        let pinned = format!("/proc/self/fd/{}", parent.as_raw_fd());
        let directory = tempfile::Builder::new()
            .prefix("conglobate-context-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(pinned)?;
        let root = File::open(directory.path())?;
        let staged = StagedContext {
            resources: StagedResources {
                directory,
                root,
                _parent: parent,
            },
        };
        for (relative, entry) in self.entries() {
            let target = staged.resources.directory.path().join(relative);
            match entry {
                ContextEntry::Directory => fs::create_dir(target)?,
                ContextEntry::File { executable, bytes } => {
                    // Ordinary checkout modes are subject to the builder umask;
                    // never normalize them to read-only to simulate a mount.
                    let mut file = OpenOptions::new()
                        .create_new(true)
                        .write(true)
                        .mode(if *executable { 0o777 } else { 0o666 })
                        .open(target)?;
                    file.write_all(bytes)?;
                }
                ContextEntry::Symlink(bytes) => {
                    std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(bytes), target)?;
                }
            }
        }
        Ok(staged)
    }
}
impl StagedContext {
    /// Read-only descriptor access, confined beneath the captured root. This
    /// does not expose the staging pathname or a writable directory descriptor.
    pub fn open(&self, path: &ContextPath) -> Result<StagedFile<'_>, ContextError> {
        let fd = rustix::fs::openat2(
            &self.resources.root,
            path.as_ref(),
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS,
        )
        .map_err(io::Error::from)?;
        let file = File::from(fd);
        if !file.metadata()?.is_file() {
            return Err(ContextError::Invalid(
                "selected checkout path is not a file",
            ));
        }
        Ok(StagedFile {
            file,
            _owner: std::marker::PhantomData,
        })
    }
    /// Explicit cleanup reports failure; Drop remains best-effort fallback.
    pub fn close(self) -> Result<(), ContextError> {
        let StagedResources {
            directory,
            root,
            _parent,
        } = self.resources;
        drop(root);
        let result = directory.close();
        drop(_parent);
        result.map_err(ContextError::from)
    }
}

/// Borrowed read authority cannot survive the consuming read-only transition.
/// No raw descriptor or metadata mutation methods are exposed.
///
/// ```compile_fail
/// use conglobate::{context::StagedContext, path::ContextPath};
/// fn cannot_freeze_with_open_reader(staged: StagedContext, path: ContextPath) {
///     let file = staged.open(&path).unwrap();
///     let frozen = staged.read_only();
///     drop(file);
/// }
/// ```
#[derive(Debug)]
pub struct StagedFile<'a> {
    file: File,
    _owner: std::marker::PhantomData<&'a StagedContext>,
}
impl StagedFile<'_> {
    pub fn metadata(&self) -> io::Result<fs::Metadata> {
        self.file.metadata()
    }
}
impl Read for StagedFile<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.file.read(bytes)
    }
}
impl Seek for StagedFile<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.file.seek(position)
    }
}
