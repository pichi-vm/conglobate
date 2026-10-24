// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::Scratch;
use crate::chain::{ChainError, ScuteExtent};
use devmap_snapshot::{Exception, Header};
use pichi_artifact::{ScuteAnnotations, ScuteDescriptor};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    os::unix::fs::FileExt,
};

/// Immutable local scute bytes and generated verity tree. No writable scratch
/// authority escapes. This proves sealing, not command success or publication.
///
#[derive(Debug)]
pub(crate) struct SealedScute<'workspace> {
    data: Scratch<'workspace>,
    hash: Scratch<'workspace>,
    descriptor: ScuteDescriptor,
    root: [u8; 32],
    parent: [u8; 32],
}
impl SealedScute<'_> {
    pub(crate) fn files(&self) -> (&File, &File) {
        (self.data.file(), self.hash.file())
    }
    pub(crate) fn descriptor(&self) -> &ScuteDescriptor {
        &self.descriptor
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn parent(&self) -> &[u8; 32] {
        &self.parent
    }
    pub(crate) fn data_reader(&self) -> ScuteReader<'_> {
        ScuteReader {
            file: self.data.file(),
            position: 0,
            length: self.descriptor.size,
        }
    }
}
/// Read/seek-only view; neither a raw file descriptor nor write access escapes.
#[derive(Debug)]
pub(crate) struct ScuteReader<'a> {
    file: &'a File,
    position: u64,
    length: u64,
}
impl<'a> ScuteReader<'a> {
    pub(crate) fn new(file: &'a File, length: u64) -> Self {
        Self {
            file,
            position: 0,
            length,
        }
    }
}
impl Read for ScuteReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self
            .length
            .saturating_sub(self.position)
            .min(buffer.len() as u64) as usize;
        if count == 0 {
            return Ok(0);
        }
        let read = self.file.read_at(&mut buffer[..count], self.position)?;
        self.position += read as u64;
        Ok(read)
    }
}
impl Seek for ScuteReader<'_> {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.position = match offset {
            SeekFrom::Start(position) => Some(position),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self.length.checked_add_signed(delta),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid scute seek"))?;
        Ok(self.position)
    }
}
impl<'w> Scratch<'w> {
    /// Seal a detached persistent snapshot store. LoopAttachment::detach must
    /// have confirmed unbound state before returning this writable authority.
    /// Only unused trailing capacity is removed; no filesystem image is copied
    /// or converted. Caller separately owns command/finalization success proofs.
    pub(crate) fn seal_snapshot(mut self, parent: [u8; 32]) -> Result<SealedScute<'w>, ChainError> {
        let capacity = self.file.metadata()?.len();
        let extent = SnapshotExtent::read(&mut self.file, capacity)?;
        self.file.set_len(extent.bytes)?;
        self.file.sync_all()?;
        self.file.rewind()?;
        let mut hash = self._workspace.scratch()?;
        let mut input = MeasuredRead {
            input: &mut self.file,
            digest: Sha256::new(),
        };
        let root = ScuteExtent::new(extent.bytes, &parent, Some(extent.bytes / 4096))?
            .format(&mut input, &mut hash)?;
        let digest = format!("sha256:{}", hex::encode(input.digest.finalize()));
        hash.sync_all()?;
        let descriptor = ScuteDescriptor {
            digest,
            size: extent.bytes,
            annotations: ScuteAnnotations {
                data_blocks: Some(extent.bytes / 4096),
                salt: hex::encode(parent),
            },
        };
        Ok(SealedScute {
            data: self,
            hash,
            root,
            descriptor,
            parent,
        })
    }
}
#[derive(Debug)]
struct MeasuredRead<R> {
    input: R,
    digest: Sha256,
}
impl<R: Read> Read for MeasuredRead<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.input.read(buffer)?;
        self.digest.update(&buffer[..read]);
        Ok(read)
    }
}
/// Application-specific used extent of our kernel-produced 4KiB snapshot store.
/// Wire decoding stays in devmap-snapshot. This is not an independent verifier
/// of every snapshot semantic: the kernel health check precedes detachment.
#[derive(Debug)]
struct SnapshotExtent {
    bytes: u64,
}
impl SnapshotExtent {
    fn read(input: &mut (impl Read + Seek), capacity: u64) -> io::Result<Self> {
        if capacity < 8192 || !capacity.is_multiple_of(4096) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid snapshot capacity",
            ));
        }
        input.rewind()?;
        let mut record = [0; 16];
        input.read_exact(&mut record)?;
        let header = Header::decode(record)?;
        if !header.valid || header.chunk_size.bytes().get() != 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid snapshot header",
            ));
        }
        let chunks = capacity / 4096;
        let stride = header.exceptions_per_chunk() + 1;
        let mut end = 2;
        for index in 0..chunks {
            let area = header.metadata_chunk_index(index)?;
            if area >= chunks {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "snapshot lacks terminating metadata",
                ));
            }
            input.seek(SeekFrom::Start(area * 4096))?;
            let mut metadata = [0; 4096];
            input.read_exact(&mut metadata)?;
            end = end.max(area + 1);
            for record in metadata.as_chunks::<16>().0 {
                let exception = Exception::decode(*record);
                if exception.new_chunk == 0 {
                    return Ok(Self { bytes: end * 4096 });
                }
                if exception.old_chunk >= crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 4096
                    || exception.new_chunk < 2
                    || exception.new_chunk >= chunks
                    || (exception.new_chunk - 1).is_multiple_of(stride)
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "snapshot exception outside data geometry",
                    ));
                }
                end = end.max(exception.new_chunk + 1);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot metadata exceeds capacity",
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[derive(Debug)]
    struct Store(Vec<u8>);
    impl Store {
        fn new(chunks: usize) -> Self {
            let mut bytes = vec![0; chunks * 4096];
            bytes[..16].copy_from_slice(
                &Header {
                    valid: true,
                    chunk_size: Default::default(),
                }
                .encode(),
            );
            Self(bytes)
        }
        fn entry(&mut self, index: usize, old_chunk: u64, new_chunk: u64) {
            let offset = 4096 + index * 16;
            self.0[offset..offset + 16].copy_from_slice(
                &Exception {
                    old_chunk,
                    new_chunk,
                }
                .encode(),
            );
        }
        fn extent(&self) -> io::Result<u64> {
            SnapshotExtent::read(&mut Cursor::new(&self.0), self.0.len() as u64)
                .map(|extent| extent.bytes)
        }
    }
    #[test]
    fn readonly_cursors_are_independent_and_allow_eof_seeks() {
        use std::io::Write;
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"layer").unwrap();
        let mut first = ScuteReader {
            file: &file,
            position: 0,
            length: 5,
        };
        let mut second = ScuteReader {
            file: &file,
            position: 0,
            length: 5,
        };
        first.seek(SeekFrom::End(-1)).unwrap();
        let mut byte = [0];
        first.read_exact(&mut byte).unwrap();
        assert_eq!(byte, *b"r");
        second.read_exact(&mut byte).unwrap();
        assert_eq!(byte, *b"l");
        first.seek(SeekFrom::Start(u64::MAX)).unwrap();
        assert_eq!(first.read(&mut byte).unwrap(), 0);
        assert!(second.seek(SeekFrom::Current(-2)).is_err());
    }
    #[test]
    fn trailing_capacity_and_full_metadata_boundary() {
        let mut store = Store::new(270);
        assert_eq!(store.extent().unwrap(), 8192);
        store.entry(0, 12, 7);
        assert_eq!(store.extent().unwrap(), 8 * 4096);
        for index in 0..256 {
            store.entry(index, index as u64, index as u64 + 2);
        }
        assert_eq!(store.extent().unwrap(), 259 * 4096);
        store.0.truncate(258 * 4096);
        assert!(store.extent().is_err());
    }
    #[test]
    fn malformed_records_never_select_an_extent() {
        for new in [1, 258, 300] {
            let mut store = Store::new(270);
            store.entry(0, 0, new);
            assert!(store.extent().is_err());
        }
        let mut store = Store::new(3);
        store.entry(
            0,
            crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 4096,
            2,
        );
        assert!(store.extent().is_err());
        store.0[4..8].fill(0);
        assert!(store.extent().is_err());
    }
}
