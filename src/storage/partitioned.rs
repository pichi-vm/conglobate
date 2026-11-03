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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Workspace;
    use std::io::{Read, Write};

    #[test]
    #[ignore = "requires root, loop devices and device mapper"]
    fn partition_views_keep_disk_offsets_in_one_snapshot() {
        let workspace = Workspace::create().unwrap();
        let sectors = NonZeroU64::new(carapace::ZERO_SIZE_SECTORS).unwrap();
        let origin = Mapping::<Created>::create().unwrap().zero(sectors).unwrap();
        for esp in [None, Some(1024 * 1024)] {
            let snapshot = Mapping::<Created>::create()
                .unwrap()
                .snapshot(
                    &origin,
                    workspace.snapshot_store().unwrap().attach().unwrap(),
                    sectors,
                )
                .unwrap();
            let layout =
                Layout::new(Architecture::current().unwrap(), 16 * 1024 * 1024, esp).unwrap();
            let views = PartitionedSnapshot::create(snapshot, &layout).unwrap();
            let mut root = views.root.open_read_write().unwrap();
            assert_eq!(root.seek(SeekFrom::End(0)).unwrap(), 16 * 1024 * 1024);
            root.seek(SeekFrom::Start(0)).unwrap();
            root.write_all(b"root").unwrap();
            drop(root);
            if let Some(esp) = &views.esp {
                let mut file = esp.open_read_write().unwrap();
                assert_eq!(file.seek(SeekFrom::End(0)).unwrap(), 1024 * 1024);
                file.seek(SeekFrom::Start(0)).unwrap();
                file.write_all(b"esp!").unwrap();
            }
            views.sync().unwrap();
            let snapshot = views.into_snapshot().unwrap();
            let mut disk_file = snapshot.open_read_only().unwrap();
            let disk = Disk::read(&mut disk_file, carapace::ZERO_SIZE_BYTES).unwrap();
            let root = disk.root(Architecture::current().unwrap()).unwrap();
            for (partition, expected) in [(Some(root), b"root"), (disk.esp().unwrap(), b"esp!")] {
                if let Some(partition) = partition {
                    disk_file
                        .seek(SeekFrom::Start(partition.start * 512))
                        .unwrap();
                    let mut bytes = [0; 4];
                    disk_file.read_exact(&mut bytes).unwrap();
                    assert_eq!(&bytes, expected);
                }
            }
            drop(disk_file);
            // Reopening consumes on-disk GPT data, not the creation layout.
            let views = PartitionedSnapshot::open(snapshot).unwrap();
            views
                .into_snapshot()
                .unwrap()
                .close()
                .unwrap()
                .detach()
                .unwrap();
        }
        origin.close().unwrap();
    }
}
