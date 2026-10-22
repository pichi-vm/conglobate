// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Mount ownership inside the builder's private mount namespace.

use crate::Error;
use nix::mount::{MntFlags, MsFlags, mount, umount, umount2};
use std::{
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

/// One build-wide RAM-backed workspace. Call only after making mount
/// propagation private. No tmpfs capacity, swap, or filesystem policy is imposed.
#[derive(Debug)]
pub struct Scratch {
    directory: tempfile::TempDir,
    _lock: std::fs::File,
}

impl Scratch {
    pub fn new() -> Result<Self, Error> {
        // Serialize discovery and creation so recovery cannot remove a new
        // mountpoint in the interval before its lifetime lock is acquired.
        let _creation_lock = scratch_creation_lock()?;
        recover_empty_scratch()?;
        let directory = tempfile::Builder::new()
            .prefix("conglobate-")
            .permissions(std::fs::Permissions::from_mode(0o700))
            .tempdir()?;
        let lock = std::fs::File::open(directory.path())?;
        lock.lock()?;
        mount(
            Some("tmpfs"),
            directory.path(),
            Some("tmpfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("mode=700"),
        )?;
        Ok(Self {
            directory,
            _lock: lock,
        })
    }
    pub fn path(&self) -> &Path {
        self.directory.path()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = umount2(self.directory.path(), MntFlags::MNT_DETACH);
    }
}

#[derive(Debug)]
pub struct Filesystem {
    path: PathBuf,
    mounted: bool,
}

impl Filesystem {
    /// Ask available kernel filesystems to recognize the volume. No userspace
    /// mount helper runs and no particular root filesystem is required.
    pub fn mount(device: &Path, path: &Path) -> Result<Self, Error> {
        std::fs::create_dir(path)?;
        let supported = std::fs::read_to_string("/proc/filesystems")?;
        for line in supported.lines() {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() != 1 {
                continue;
            }
            match mount(
                Some(device),
                path,
                Some(fields[0]),
                MsFlags::empty(),
                None::<&str>,
            ) {
                Ok(()) => {
                    return Ok(Self {
                        path: path.to_owned(),
                        mounted: true,
                    });
                }
                Err(nix::errno::Errno::EINVAL | nix::errno::Errno::ENODEV) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(Error::Invalid(
            "no available kernel filesystem recognizes the root volume".into(),
        ))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Finalization must successfully unmount before the device can be sealed.
    pub fn finish(mut self) -> Result<(), Error> {
        umount(&self.path)?;
        self.mounted = false;
        Ok(())
    }
}

impl Drop for Filesystem {
    fn drop(&mut self) {
        if self.mounted {
            let _ = umount2(&self.path, MntFlags::MNT_DETACH);
        }
    }
}

/// A crashed private mount namespace leaves an empty underlying directory.
/// Only remove root-owned empty mountpoints after obtaining the lifetime lock.
fn recover_empty_scratch() -> Result<(), Error> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    for entry in std::fs::read_dir(std::env::temp_dir())? {
        let entry = entry?;
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with("conglobate-")
        {
            continue;
        }
        let metadata = match entry.path().symlink_metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o777 != 0o700 {
            continue;
        }
        let Ok(file) = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(
                nix::fcntl::OFlag::O_NOFOLLOW.bits() | nix::fcntl::OFlag::O_DIRECTORY.bits(),
            )
            .open(entry.path())
        else {
            continue;
        };
        if file.try_lock().is_err() {
            continue;
        }
        let current = file.metadata()?;
        if current.ino() != metadata.ino() || current.dev() != metadata.dev() {
            continue;
        }
        if std::fs::read_dir(entry.path())?.next().is_some() {
            continue;
        }
        if let Some(lease) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.strip_prefix("conglobate-"))
        {
            crate::snapshot::recover(lease)?;
        }
        // rmdir never removes contents, symlinks, or mounted/busy directories.
        let _ = std::fs::remove_dir(entry.path());
    }
    Ok(())
}

fn scratch_creation_lock() -> Result<std::fs::File, Error> {
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let path = std::env::temp_dir().join(".conglobate-scratch.lock");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(nix::fcntl::OFlag::O_NOFOLLOW.bits())
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o777 != 0o600 {
        return Err(Error::Invalid(
            "scratch creation lock is not a private root-owned file".into(),
        ));
    }
    file.lock()?;
    Ok(file)
}
