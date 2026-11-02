// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Architecture, Disk, Partition, SECTOR_SIZE, bounded::Bounded};
use std::io::{self, Read, Seek, Write};

const ALIGNMENT: u64 = 2048;
const BACKUP_SECTORS: u64 = 40;

/// A root partition and optional EFI System Partition with 1 MiB alignment.
/// Capacities describe partition contents; table and alignment space is extra.
#[derive(Debug, Clone)]
pub(crate) struct Layout {
    architecture: Architecture,
    root: Partition,
    esp: Option<Partition>,
    sectors: u64,
}

impl Layout {
    /// Plan a new disk without acquiring storage or writing bytes.
    /// Capacities must be positive multiples of 4096 bytes. Filesystem-specific
    /// minimum sizes are the formatter's responsibility, not a GPT property.
    pub(crate) fn new(
        architecture: Architecture,
        root_bytes: u64,
        esp_bytes: Option<u64>,
    ) -> io::Result<Self> {
        let esp = esp_bytes
            .map(Self::partition_sectors)
            .transpose()?
            .map(|sectors| Partition {
                start: ALIGNMENT,
                sectors,
            });
        let root = Partition {
            start: match esp {
                Some(esp) => Self::align(esp.start + esp.sectors)?,
                None => ALIGNMENT,
            },
            sectors: Self::partition_sectors(root_bytes)?,
        };
        let end = root
            .start
            .checked_add(root.sectors)
            .and_then(|end| end.checked_add(BACKUP_SECTORS))
            .ok_or_else(|| Self::invalid("disk layout size overflow"))?;
        let sectors = Self::align(end)?;
        if sectors > carapace::ZERO_SIZE_SECTORS {
            return Err(Self::invalid(
                "disk layout exceeds the carapace backing extent",
            ));
        }
        Ok(Self {
            architecture,
            root,
            esp,
            sectors,
        })
    }

    /// Entire logical disk size, including both GPT copies, in bytes.
    pub(crate) fn bytes(&self) -> u64 {
        self.sectors * 512
    }

    /// Write partition metadata into a new disk or snapshot, then validate it.
    ///
    /// `capacity` is the available backing extent in bytes. It may be larger
    /// than the planned disk. The writer touches only the protective MBR,
    /// headers and partition tables; it does not format or clear partitions.
    /// It creates fresh disk and partition UUIDs. The caller must discard the
    /// new output on any error and flush storage before publishing it.
    pub(crate) fn write<D: Read + Write + Seek + std::fmt::Debug>(
        &self,
        writer: &mut D,
        capacity: u64,
    ) -> io::Result<Disk> {
        if capacity < self.bytes()
            || capacity > carapace::ZERO_SIZE_BYTES
            || !capacity.is_multiple_of(SECTOR_SIZE)
        {
            return Err(Self::invalid(
                "disk layout does not fit the backing capacity",
            ));
        }
        let mut device = Bounded::new(writer, self.bytes())?;
        device.write_all(&[0; 4096])?;
        device.seek(std::io::SeekFrom::Start(0))?;
        gpt::mbr::ProtectiveMBR::with_lb_size(
            u32::try_from(self.sectors / 8 - 1).unwrap_or(u32::MAX),
        )
        .overwrite_lba0(&mut device)
        .map_err(io::Error::other)?;
        let mut disk = gpt::GptConfig::new()
            .writable(true)
            .logical_block_size(gpt::disk::LogicalBlockSize::Lb4096)
            .create_from_device(device, None)
            .map_err(io::Error::other)?;
        if let Some(esp) = self.esp {
            disk.add_partition_at(
                "esp",
                1,
                esp.start / 8,
                esp.sectors / 8,
                gpt::partition_types::EFI,
                0,
            )
            .map_err(io::Error::other)?;
        }
        disk.add_partition_at(
            "root",
            if self.esp.is_some() { 2 } else { 1 },
            self.root.start / 8,
            self.root.sectors / 8,
            self.architecture.partition_type(),
            0,
        )
        .map_err(io::Error::other)?;
        disk.write().map_err(io::Error::other)?.flush()?;
        Disk::read(writer, capacity)
    }

    fn partition_sectors(bytes: u64) -> io::Result<u64> {
        if bytes == 0 || !bytes.is_multiple_of(4096) || bytes > carapace::ZERO_SIZE_BYTES {
            return Err(Self::invalid(
                "partition capacity must be positive, 4096-byte aligned and fit a carapace",
            ));
        }
        Ok(bytes / 512)
    }

    fn align(sectors: u64) -> io::Result<u64> {
        sectors
            .checked_add(ALIGNMENT - 1)
            .map(|rounded| rounded / ALIGNMENT * ALIGNMENT)
            .ok_or_else(|| Self::invalid("disk alignment overflow"))
    }

    fn invalid(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn writes_aligned_native_roots_and_optional_esp() {
        for architecture in [Architecture::X86_64, Architecture::Aarch64] {
            for esp in [None, Some(1024 * 1024), Some(1024 * 1024 + 4096)] {
                let layout = Layout::new(architecture, 2 * 1024 * 1024, esp).unwrap();
                let mut source = Cursor::new(Vec::new());
                let disk = layout
                    .write(&mut source, carapace::ZERO_SIZE_BYTES)
                    .unwrap();
                let root = disk.root(architecture).unwrap();
                assert_eq!(root.sectors, 2 * 1024 * 1024 / 512);
                assert_eq!(root.start % ALIGNMENT, 0);
                assert_eq!(source.get_ref().len() as u64, layout.bytes());
                assert_eq!(disk.esp().unwrap().map(|p| p.sectors * 512), esp);
                let other = match architecture {
                    Architecture::X86_64 => Architecture::Aarch64,
                    Architecture::Aarch64 => Architecture::X86_64,
                };
                assert!(disk.root(other).is_err());
            }
        }
    }

    #[test]
    fn metadata_writes_preserve_partition_contents_and_backing_tail() {
        let layout = Layout::new(Architecture::X86_64, 1024 * 1024, Some(1024 * 1024)).unwrap();
        let capacity = layout.bytes() + 4096;
        let mut source = Cursor::new(vec![0x5a; capacity as usize]);
        let disk = layout.write(&mut source, capacity).unwrap();
        for partition in [
            disk.root(Architecture::X86_64).unwrap(),
            disk.esp().unwrap().unwrap(),
        ] {
            let start = (partition.start * 512) as usize;
            let end = start + (partition.sectors * 512) as usize;
            assert!(
                source.get_ref()[start..end]
                    .iter()
                    .all(|byte| *byte == 0x5a)
            );
        }
        assert!(
            source.get_ref()[layout.bytes() as usize..]
                .iter()
                .all(|byte| *byte == 0x5a)
        );
    }

    #[test]
    fn rejects_invalid_sizes_without_writing() {
        for size in [0, 512, u64::MAX, carapace::ZERO_SIZE_BYTES] {
            assert!(Layout::new(Architecture::X86_64, size, None).is_err());
            assert!(Layout::new(Architecture::X86_64, 1024 * 1024, Some(size)).is_err());
        }
        let layout = Layout::new(Architecture::X86_64, 1024 * 1024, None).unwrap();
        let mut source = Cursor::new(vec![0x5a; 512]);
        for capacity in [
            0,
            layout.bytes() - 512,
            carapace::ZERO_SIZE_BYTES + 512,
            layout.bytes() + 1,
        ] {
            assert!(layout.write(&mut source, capacity).is_err());
            assert_eq!(source.get_ref(), &[0x5a; 512]);
        }
    }

    #[test]
    fn disk_and_partition_ids_are_new_and_distinct() {
        let layout = Layout::new(Architecture::X86_64, 1024 * 1024, Some(1024 * 1024)).unwrap();
        let mut ids = std::collections::BTreeSet::new();
        for _ in 0..2 {
            let mut source = Cursor::new(Vec::new());
            layout.write(&mut source, layout.bytes()).unwrap();
            let disk = gpt::GptConfig::new()
                .only_valid_headers(true)
                .logical_block_size(gpt::disk::LogicalBlockSize::Lb4096)
                .open_from_device(source)
                .unwrap();
            let disk_id = disk.primary_header().unwrap().disk_guid;
            assert!(!disk_id.is_nil());
            assert!(ids.insert(disk_id));
            for partition in disk.partitions().values() {
                assert!(!partition.part_guid.is_nil());
                assert!(ids.insert(partition.part_guid));
            }
        }
    }
}
