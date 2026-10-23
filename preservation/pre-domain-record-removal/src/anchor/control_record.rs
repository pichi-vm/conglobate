// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::RecoveryDirectory;
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::PathBuf,
};

/// Bounded private control-record storage under the cooperative directory lease.
/// Typed callers must verify resource identity before saving/removing records.
#[derive(Debug)]
pub(super) struct ControlRecords<'a>(&'a RecoveryDirectory);
impl<'a> ControlRecords<'a> {
    pub(super) fn new(directory: &'a RecoveryDirectory) -> Self {
        Self(directory)
    }
    fn filename(&self, name: &str) -> io::Result<String> {
        self.0.lease.namespace.verify_current()?;
        if name.is_empty()
            || name.len() > 96
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid control record name",
            ));
        }
        Ok(format!("{name}.cbor"))
    }
    pub(super) fn save(&self, name: &str, bytes: &[u8]) -> io::Result<()> {
        let filename = self.filename(name)?;
        if bytes.len() > 256 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "oversized control record",
            ));
        }
        let root = PathBuf::from(format!(
            "/proc/self/fd/{}",
            self.0.lease.directory.as_raw_fd()
        ));
        let mut staged = tempfile::Builder::new()
            .prefix("record-")
            .tempfile_in(&root)?;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        staged.write_all(bytes)?;
        staged.as_file().sync_all()?;
        staged
            .persist_noclobber(root.join(filename))
            .map_err(|e| e.error)?;
        self.0.lease.directory.sync_all()
    }
    pub(super) fn load(&self, name: &str) -> io::Result<Vec<u8>> {
        let filename = self.filename(name)?;
        let file = File::from(rustix::fs::openat(
            &self.0.lease.directory,
            filename,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )?);
        let m = file.metadata()?;
        if !m.is_file() || m.uid() != 0 || m.mode() & 0o777 != 0o600 || m.len() > 256 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid control record file",
            ));
        }
        let mut bytes = Vec::new();
        file.take(257).read_to_end(&mut bytes)?;
        if bytes.len() > 256 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "oversized control record",
            ));
        }
        Ok(bytes)
    }
    pub(super) fn remove(&self, name: &str) -> io::Result<()> {
        let filename = self.filename(name)?;
        rustix::fs::unlinkat(
            &self.0.lease.directory,
            filename,
            rustix::fs::AtFlags::empty(),
        )?;
        self.0.lease.directory.sync_all()
    }
}
