// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CommandDomain, DomainTarget};
use crate::anchor::RecoveryDirectory;
use std::{fs::File, io, os::unix::fs::MetadataExt, path::Path};

/// Validated description only; decoding never grants cancellation authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainRecord {
    name: String,
    parent_device: u64,
    parent_inode: u64,
    device: u64,
    inode: u64,
}
impl DomainRecord {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub(crate) fn valid_name(name: &str) -> bool {
        name.strip_prefix("conglobate-command-")
            .is_some_and(|suffix| {
                !suffix.is_empty()
                    && suffix.len() <= 64
                    && suffix.bytes().all(|b| b.is_ascii_alphanumeric())
            })
    }
    /// Canonical CBOR [1, name, parent-device, parent-inode, device, inode].
    pub fn to_cbor(&self) -> Vec<u8> {
        let mut e = minicbor::Encoder::new(Vec::new());
        e.array(6)
            .expect("Vec writer is infallible")
            .u8(1)
            .expect("Vec writer is infallible")
            .str(&self.name)
            .expect("Vec writer is infallible");
        for n in [
            self.parent_device,
            self.parent_inode,
            self.device,
            self.inode,
        ] {
            e.u64(n).expect("Vec writer is infallible");
        }
        e.into_writer()
    }
    fn parent(&self, path: &Path) -> io::Result<File> {
        let file = DomainTarget::open_parent(path)?;
        self.check_parent(&file)?;
        Ok(file)
    }
    fn check_parent(&self, file: &File) -> io::Result<()> {
        let m = file.metadata()?;
        if (m.dev(), m.ino()) != (self.parent_device, self.parent_inode) {
            return Err(io::Error::other("domain parent identity mismatch"));
        }
        Ok(())
    }
    fn open(&self, parent: &File) -> io::Result<File> {
        self.check_parent(parent)?;
        let file = File::from(rustix::fs::openat(
            parent,
            self.name.as_str(),
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::DIRECTORY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?);
        let m = file.metadata()?;
        if (m.dev(), m.ino()) != (self.device, self.inode)
            || m.uid() != 0
            || m.mode() & 0o777 != 0o700
            || rustix::fs::fstatfs(&file)?.f_type as u64 != 0x6367_7270
        {
            return Err(io::Error::other("domain identity or permissions mismatch"));
        }
        Ok(file)
    }
    pub(crate) fn verify_absent(&self, parent: &Path) -> io::Result<()> {
        let parent = self.parent(parent)?;
        match rustix::fs::openat(
            &parent,
            self.name.as_str(),
            rustix::fs::OFlags::PATH | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NOFOLLOW,
            rustix::fs::Mode::empty(),
        ) {
            Err(rustix::io::Errno::NOENT) => Ok(()),
            Err(error) => Err(error.into()),
            Ok(_) => Err(io::Error::other("domain target still exists")),
        }
    }
}
impl TryFrom<&[u8]> for DomainRecord {
    type Error = io::Error;
    fn try_from(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() > 256 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "domain record too large",
            ));
        }
        let mut d = minicbor::Decoder::new(bytes);
        let record = (|| -> Result<Self, minicbor::decode::Error> {
            if d.array()? != Some(6) || d.u8()? != 1 {
                return Err(minicbor::decode::Error::message(
                    "unsupported domain record",
                ));
            }
            let name = d.str()?;
            if !Self::valid_name(name) {
                return Err(minicbor::decode::Error::message("invalid domain name"));
            }
            let record = Self {
                name: name.into(),
                parent_device: d.u64()?,
                parent_inode: d.u64()?,
                device: d.u64()?,
                inode: d.u64()?,
            };
            if record.parent_inode == 0 || record.inode == 0 || d.position() != bytes.len() {
                return Err(minicbor::decode::Error::message(
                    "invalid domain identity or trailing bytes",
                ));
            }
            Ok(record)
        })()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        if record.to_cbor() != bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "noncanonical domain record",
            ));
        }
        Ok(record)
    }
}
impl CommandDomain {
    pub(crate) fn record_for(&self, recovery: &RecoveryDirectory) -> io::Result<DomainRecord> {
        self.target.lease.verify_current()?;
        if !self.target.lease.belongs_to(recovery) {
            return Err(io::Error::other("domain belongs to another recovery lease"));
        }
        let p = self.target.parent.metadata()?;
        let m = self.root.metadata()?;
        let record = DomainRecord {
            name: self
                .target
                .name
                .to_str()
                .expect("generated ASCII name")
                .into(),
            parent_device: p.dev(),
            parent_inode: p.ino(),
            device: m.dev(),
            inode: m.ino(),
        };
        drop(record.open(&self.target.parent)?);
        Ok(record)
    }
    pub(crate) fn recover_record(
        recovery: &RecoveryDirectory,
        parent: &Path,
        record: DomainRecord,
    ) -> io::Result<Self> {
        let lease = recovery.retain()?;
        let parent = record.parent(parent)?;
        let root = record.open(&parent)?;
        let domain = Self {
            root,
            target: DomainTarget {
                parent,
                name: record.name.into(),
                lease,
            },
        };
        if domain.text("cgroup.type")?.trim() != "domain" {
            return Err(io::Error::other("recovered cgroup is not a domain"));
        }
        Ok(domain)
    }
}
