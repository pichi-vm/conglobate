// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Header, MbrEntry, Partition, ROOT_PARTITION_TYPE, Record, SECTOR_SIZE};
use crc::{CRC_32_ISO_HDLC, Crc};
use std::io::{self, Seek, SeekFrom, Write};
use uuid::Uuid;
use zerocopy::IntoBytes as _;

const ALIGNMENT: u64 = 2048; // 1 MiB in device-mapper sectors.

/// One native root with 1 MiB alignment, plus both copies of its GPT metadata.
#[derive(Debug)]
pub(crate) struct Layout {
    root: Partition,
    sectors: u64,
}

impl Layout {
    /// The capacity describes partition contents, excluding GPT/alignment space.
    pub(crate) fn new(root_bytes: u64) -> io::Result<Self> {
        if root_bytes == 0
            || !root_bytes.is_multiple_of(SECTOR_SIZE)
            || root_bytes > carapace::ZERO_SIZE_BYTES
        {
            return Err(Self::invalid(
                "root capacity must be positive, 4096-byte aligned and fit a carapace",
            ));
        }
        let root = Partition {
            start: ALIGNMENT,
            sectors: root_bytes / 512,
        };
        // The Carapace limit bounds this addition. Reserve four GPT blocks for
        // the backup table and one for its header, then round to 1 MiB.
        let sectors = (root.start + root.sectors + 40).next_multiple_of(ALIGNMENT);
        if sectors > carapace::ZERO_SIZE_SECTORS {
            return Err(Self::invalid(
                "disk layout exceeds the carapace backing extent",
            ));
        }
        Ok(Self { root, sectors })
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.sectors * 512
    }

    /// Write only the protective MBR, headers and partition tables. Partition
    /// contents and any backing extent beyond this logical disk are untouched.
    /// The caller must discard the new output on error and sync before use.
    pub(crate) fn write(&self, writer: &mut (impl Write + Seek), capacity: u64) -> io::Result<()> {
        if capacity < self.bytes()
            || capacity > carapace::ZERO_SIZE_BYTES
            || !capacity.is_multiple_of(SECTOR_SIZE)
        {
            return Err(Self::invalid(
                "disk layout does not fit the backing capacity",
            ));
        }
        let end = self.sectors / 8 - 1;
        let mut mbr = [0; 4096];
        let protective = MbrEntry {
            boot_indicator: 0,
            start_chs: [0, 2, 0],
            os_type: 0xee,
            end_chs: [0xff; 3],
            start: 1.into(),
            size: u32::try_from(end).unwrap_or(u32::MAX).into(),
        };
        mbr[446..462].copy_from_slice(protective.as_bytes());
        mbr[510..512].copy_from_slice(&[0x55, 0xaa]);

        let root = Record {
            kind: ROOT_PARTITION_TYPE.to_bytes_le(),
            identity: Uuid::new_v4().to_bytes_le(),
            start: (self.root.start / 8).into(),
            end: ((self.root.start + self.root.sectors) / 8 - 1).into(),
            attributes: 0.into(),
            name: [0; 72],
        };
        let mut table = [0; 128 * 128];
        table[..128].copy_from_slice(root.as_bytes());
        let crc = Crc::<u32>::new(&CRC_32_ISO_HDLC);
        let table_crc = crc.checksum(&table);
        let disk_guid = Uuid::new_v4().to_bytes_le();
        let mut write = |lba, bytes: &[u8]| {
            writer.seek(SeekFrom::Start(lba * SECTOR_SIZE))?;
            writer.write_all(bytes)
        };
        write(0, &mbr)?;
        write(2, &table)?;
        write(end - 4, &table)?;
        for (current, backup, part_start) in [(1, end, 2), (end, 1, end - 4)] {
            let mut header = Header {
                signature: *b"EFI PART",
                revision: 0x0001_0000.into(),
                size: 92.into(),
                crc32: 0.into(),
                reserved: 0.into(),
                current_lba: current.into(),
                backup_lba: backup.into(),
                first_usable: 6.into(),
                last_usable: (end - 5).into(),
                disk_guid,
                part_start: part_start.into(),
                num_parts: 128.into(),
                part_size: 128.into(),
                crc32_parts: table_crc.into(),
            };
            header.crc32 = crc.checksum(header.as_bytes()).into();
            let mut block = [0; 4096];
            block[..92].copy_from_slice(header.as_bytes());
            write(current, &block)?;
        }
        writer.flush()
    }

    fn invalid(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disk::Disk;
    use std::io::Cursor;

    #[test]
    fn writes_aligned_native_roots_readable_by_an_independent_parser() {
        for size in [4096, 1024 * 1024, 1024 * 1024 + 4096] {
            let layout = Layout::new(size).unwrap();
            let mut source = Cursor::new(Vec::new());
            layout
                .write(&mut source, carapace::ZERO_SIZE_BYTES)
                .unwrap();
            let disk =
                Disk::read(&mut source, carapace::ZERO_SIZE_BYTES, ROOT_PARTITION_TYPE).unwrap();
            assert_eq!(disk.root.start, ALIGNMENT);
            assert_eq!(disk.root.sectors * 512, size);
            assert_eq!(source.get_ref().len() as u64, layout.bytes());
            let independent = gpt::GptConfig::new()
                .only_valid_headers(true)
                .logical_block_size(gpt::disk::LogicalBlockSize::Lb4096)
                .open_from_device(source)
                .unwrap();
            assert_eq!(independent.partitions().len(), 1);
            let root = &independent.partitions()[&1];
            assert_eq!(root.first_lba * 8, disk.root.start);
            assert_eq!((root.last_lba - root.first_lba + 1) * 4096, size);
            assert_eq!(root.part_type_guid.guid, ROOT_PARTITION_TYPE);
        }
    }

    #[test]
    fn metadata_writes_preserve_partition_contents_and_backing_tail() {
        let layout = Layout::new(1024 * 1024).unwrap();
        let capacity = layout.bytes() + 4096;
        let mut source = Cursor::new(vec![0x5a; capacity as usize]);
        layout.write(&mut source, capacity).unwrap();
        let disk = Disk::read(&mut source, capacity, ROOT_PARTITION_TYPE).unwrap();
        let start = (disk.root.start * 512) as usize;
        let end = start + (disk.root.sectors * 512) as usize;
        assert!(
            source.get_ref()[start..end]
                .iter()
                .all(|byte| *byte == 0x5a)
        );
        assert!(
            source.get_ref()[layout.bytes() as usize..]
                .iter()
                .all(|byte| *byte == 0x5a)
        );
    }

    #[test]
    fn rejects_invalid_sizes_without_writing() {
        for size in [0, 512, u64::MAX, carapace::ZERO_SIZE_BYTES] {
            assert!(Layout::new(size).is_err());
        }
        let layout = Layout::new(1024 * 1024).unwrap();
        let mut source = Cursor::new(vec![0x5a; 512]);
        for capacity in [
            0,
            layout.bytes() - 512,
            carapace::ZERO_SIZE_BYTES + 4096,
            layout.bytes() + 1,
        ] {
            assert!(layout.write(&mut source, capacity).is_err());
            assert_eq!(source.get_ref(), &[0x5a; 512]);
        }
    }

    #[test]
    fn disk_and_partition_ids_are_new_and_distinct() {
        let layout = Layout::new(1024 * 1024).unwrap();
        let mut ids = std::collections::BTreeSet::new();
        for _ in 0..2 {
            let mut source = Cursor::new(Vec::new());
            layout.write(&mut source, layout.bytes()).unwrap();
            let disk = gpt::GptConfig::new()
                .only_valid_headers(true)
                .logical_block_size(gpt::disk::LogicalBlockSize::Lb4096)
                .open_from_device(source)
                .unwrap();
            for id in [
                disk.primary_header().unwrap().disk_guid,
                disk.partitions()[&1].part_guid,
            ] {
                assert!(!id.is_nil());
                assert!(ids.insert(id));
            }
        }
    }
}
