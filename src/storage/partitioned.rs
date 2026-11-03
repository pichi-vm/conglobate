// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Active, Created, Mapping, SnapshotMapping};
use crate::disk::{Architecture, Disk, Layout, Partition};
use std::{
    fs::File,
    io::{self, Seek, SeekFrom},
    num::NonZeroU64,
};

/// Partition views ABOVE a writable whole-disk snapshot. All writes retain
/// their disk offsets in one COW store. Explicit release precedes sealing.
#[derive(Debug)]
pub(crate) struct PartitionedSnapshot<'origin, 'workspace> {
    root: Mapping<Active>,
    esp: Option<Mapping<Active>>,
    snapshot: SnapshotMapping<'origin, 'workspace>,
}

impl<'o, 'w> PartitionedSnapshot<'o, 'w> {
    pub(crate) fn create(snapshot: SnapshotMapping<'o, 'w>, layout: &Layout) -> io::Result<Self> {
        let mut file = snapshot.open_read_write()?;
        let capacity = Self::capacity(&mut file)?;
        layout.write(&mut file, capacity)?;
        file.sync_all()?;
        drop(file);
        Self::open(snapshot)
    }

    pub(crate) fn open(snapshot: SnapshotMapping<'o, 'w>) -> io::Result<Self> {
        let mut file = snapshot.open_read_only()?;
        let capacity = Self::capacity(&mut file)?;
        let disk = Disk::read(&mut file, capacity)?;
        let root = disk.root(Architecture::current()?)?;
        let esp = disk.esp()?;
        let root = Self::view(&snapshot, root)?;
        let esp = match esp
            .map(|partition| Self::view(&snapshot, partition))
            .transpose()
        {
            Ok(esp) => esp,
            Err(error) => {
                if let Err(cleanup) = root.close() {
                    std::mem::forget(snapshot);
                    return Err(io::Error::other(format!(
                        "{error}; root view cleanup: {cleanup}"
                    )));
                }
                return Err(error);
            }
        };
        Ok(Self {
            root,
            esp,
            snapshot,
        })
    }

    fn capacity(file: &mut File) -> io::Result<u64> {
        if rustix::fs::ioctl_blksszget(&*file)? != 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "partitioned images require 4096-byte logical sectors",
            ));
        }
        file.seek(SeekFrom::End(0))
    }

    fn view(
        snapshot: &SnapshotMapping<'_, '_>,
        partition: Partition,
    ) -> io::Result<Mapping<Active>> {
        let target = devmap_linux::targets::Linear {
            device: snapshot.id(),
            offset: partition.start,
        };
        Mapping::<Created>::create()?.activate(
            NonZeroU64::new(partition.sectors).expect("validated nonempty partition"),
            &target,
            false,
        )
    }

    pub(crate) fn root_id(&self) -> devmap_linux::DevId {
        self.root.id()
    }

    pub(crate) fn esp_id(&self) -> Option<devmap_linux::DevId> {
        self.esp.as_ref().map(Mapping::id)
    }

    pub(super) fn open_root(&self) -> io::Result<File> {
        self.root.open_read_only()
    }

    pub(super) fn open_esp(&self) -> io::Result<File> {
        self.esp
            .as_ref()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "stage requests an ESP mount but the image has no ESP",
                )
            })?
            .open_read_only()
    }

    pub(crate) fn validate(&self) -> io::Result<()> {
        self.snapshot.usage().map(|_| ())
    }

    /// Flush trusted formatter writes before accepting a completed format.
    /// Filesystem mounts have their separate syncfs/unmount protocol.
    pub(crate) fn sync(&self) -> io::Result<()> {
        self.root.open_read_only()?.sync_all()?;
        if let Some(esp) = &self.esp {
            esp.open_read_only()?.sync_all()?;
        }
        self.validate()
    }

    pub(crate) fn into_snapshot(self) -> io::Result<SnapshotMapping<'o, 'w>> {
        let Self {
            root,
            esp,
            snapshot,
        } = self;
        if let Err(error) = root.close() {
            std::mem::forget((esp, snapshot));
            return Err(error);
        }
        if let Some(esp) = esp
            && let Err(error) = esp.close()
        {
            std::mem::forget(snapshot);
            return Err(error);
        }
        Ok(snapshot)
    }
}
