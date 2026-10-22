// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{NamespacePin, NamespaceRecord, RecoveryDirectory};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::PathBuf,
};

impl NamespaceRecord {
    // Bounded single-entry control record, not the build inventory/lease schema.
    const MAX_BYTES: u64 = 256;
    fn valid_name(name: &str) -> bool {
        name.strip_prefix("namespace-").is_some_and(|suffix| {
            !suffix.is_empty()
                && suffix.len() <= 64
                && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
        })
    }
    /// Deterministic CBOR [version=1, pin-name, nsfs-device, namespace-inode].
    /// The resulting data contains no authority or proof of a live mount.
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut encoder = minicbor::Encoder::new(Vec::new());
        encoder
            .array(4)
            .expect("Vec writer is infallible")
            .u8(1)
            .expect("Vec writer is infallible")
            .str(self.name.to_str().expect("validated ASCII pin name"))
            .expect("Vec writer is infallible")
            .u64(self.device)
            .expect("Vec writer is infallible")
            .u64(self.inode)
            .expect("Vec writer is infallible");
        encoder.into_writer()
    }
}
impl TryFrom<&[u8]> for NamespaceRecord {
    type Error = io::Error;
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() as u64 > Self::MAX_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "namespace record too large",
            ));
        }
        let mut decoder = minicbor::Decoder::new(bytes);
        let result = (|| -> Result<Self, minicbor::decode::Error> {
            if decoder.array()? != Some(4) || decoder.u8()? != 1 {
                return Err(minicbor::decode::Error::message(
                    "unsupported namespace record",
                ));
            }
            let name = decoder.str()?;
            if !Self::valid_name(name) {
                return Err(minicbor::decode::Error::message(
                    "invalid namespace pin name",
                ));
            }
            let device = decoder.u64()?;
            let inode = decoder.u64()?;
            if inode == 0 || inode > u32::MAX as u64 || decoder.position() != bytes.len() {
                return Err(minicbor::decode::Error::message(
                    "invalid namespace identity or trailing data",
                ));
            }
            Ok(Self {
                name: name.into(),
                device,
                inode,
            })
        })()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if result.to_cbor() != bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "noncanonical namespace record",
            ));
        }
        Ok(result)
    }
}
impl RecoveryDirectory {
    /// Remove a matching record only after its pin and underlying target are
    /// gone. Policy/containment teardown remains the enclosing owner's duty.
    pub fn remove_namespace_record(&self, record: &NamespaceRecord) -> io::Result<()> {
        let name = record.name.to_str().expect("validated ASCII name");
        if self.load_namespace(name)? != *record {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "namespace record identity mismatch",
            ));
        }
        match rustix::fs::openat(
            &self.directory,
            &record.name,
            rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        ) {
            Err(rustix::io::Errno::NOENT) => {}
            Err(error) => return Err(error.into()),
            Ok(_) => return Err(io::Error::other("namespace pin target still exists")),
        }
        rustix::fs::unlinkat(
            &self.directory,
            format!("{name}.cbor"),
            rustix::fs::AtFlags::empty(),
        )?;
        self.directory.sync_all()
    }

    /// Publish one verified namespace record without replacing an existing one.
    /// An error after publication can leave a complete record; fail the build
    /// and inspect under this lease rather than assuming nothing was written.
    pub fn save_namespace(&self, record: &NamespaceRecord) -> io::Result<()> {
        // Recheck the mount, not just the syntactic record. Dropping this view
        // leaves the kernel pin intact and releases only its shared lease ref.
        drop(NamespacePin::recover(self, record.clone())?);
        let root = PathBuf::from(format!("/proc/self/fd/{}", self.directory.as_raw_fd()));
        let mut staged = tempfile::Builder::new()
            .prefix("record-")
            .tempfile_in(&root)?;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
        staged.write_all(&record.to_cbor())?;
        staged.as_file().sync_all()?;
        let destination = root.join(format!(
            "{}.cbor",
            record.name.to_str().expect("validated ASCII name")
        ));
        staged
            .persist_noclobber(destination)
            .map_err(|error| error.error)?;
        self.directory.sync_all()
    }

    /// Read a bounded validated description. Recovery must still verify its
    /// mounted identity. Missing or corrupt records never authorize cleanup.
    pub fn load_namespace(&self, name: &str) -> io::Result<NamespaceRecord> {
        if !NamespaceRecord::valid_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid namespace pin name",
            ));
        }
        let file = File::from(rustix::fs::openat(
            &self.directory,
            format!("{name}.cbor"),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )?);
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != 0
            || metadata.mode() & 0o777 != 0o600
            || metadata.len() > NamespaceRecord::MAX_BYTES
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid namespace record file",
            ));
        }
        let mut bytes = Vec::new();
        file.take(NamespaceRecord::MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
        let record = NamespaceRecord::try_from(bytes.as_slice())?;
        if record.name != name {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "namespace record name mismatch",
            ));
        }
        Ok(record)
    }
}
