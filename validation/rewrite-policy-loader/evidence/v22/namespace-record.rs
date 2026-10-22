// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::control_record::ControlRecords;
use super::{NamespacePin, NamespaceRecord, RecoveryDirectory};
use std::io;

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
        self.lease.namespace.verify_current()?;
        let name = record.name.to_str().expect("validated ASCII name");
        if self.load_namespace(name)? != *record {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "namespace record identity mismatch",
            ));
        }
        match rustix::fs::openat(
            &self.lease.directory,
            &record.name,
            rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        ) {
            Err(rustix::io::Errno::NOENT) => {}
            Err(error) => return Err(error.into()),
            Ok(_) => return Err(io::Error::other("namespace pin target still exists")),
        }
        rustix::fs::unlinkat(
            &self.lease.directory,
            format!("{name}.cbor"),
            rustix::fs::AtFlags::empty(),
        )?;
        self.lease.directory.sync_all()
    }

    /// Publish one verified namespace record without replacing an existing one.
    /// An error after publication can leave a complete record; fail the build
    /// and inspect under this lease rather than assuming nothing was written.
    pub fn save_namespace(&self, record: &NamespaceRecord) -> io::Result<()> {
        // Recheck the mount, not just the syntactic record. Dropping this view
        // leaves the kernel pin intact and releases only its shared lease ref.
        drop(NamespacePin::recover(self, record.clone())?);
        ControlRecords::new(self).save(
            record.name.to_str().expect("validated ASCII name"),
            &record.to_cbor(),
        )
    }

    /// Read a bounded validated description. Recovery must still verify its
    /// mounted identity. Missing or corrupt records never authorize cleanup.
    pub fn load_namespace(&self, name: &str) -> io::Result<NamespaceRecord> {
        self.lease.namespace.verify_current()?;
        if !NamespaceRecord::valid_name(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid namespace pin name",
            ));
        }
        let bytes = ControlRecords::new(self).load(name)?;
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
