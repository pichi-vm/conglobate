// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use devmap_snapshot::Exception;
use std::{
    fs::File,
    io::{self, Read},
    os::unix::fs::FileExt,
};

/// Final-order bytes of a validated, detached 4-KiB persistent snapshot.
/// Planning repacks exception metadata in place, always behind the scan, and
/// leaves source data chunks untouched. No separate index or compacted COW is
/// needed before verity consumes the stream. The origin is the same immutable
/// chain used by the writer.
#[derive(Debug)]
pub(super) struct CompactSnapshot<'a> {
    source: &'a File,
    kept: u64,
    chunks: u64,
    next: u64,
    source_chunks: [u64; 256],
    buffer: [u8; 4096],
    position: usize,
}
impl<'a> CompactSnapshot<'a> {
    pub(super) fn new(
        source: &'a mut File,
        mut read_origin: impl FnMut(u64, &mut [u8; 4096]) -> io::Result<()>,
    ) -> io::Result<Self> {
        let mut block = [0; 4096];
        let mut origin = [0; 4096];
        let mut output = [0; 4096];
        let mut kept = 0;
        for area in 0.. {
            let mut metadata = [0; 4096];
            source.read_exact_at(&mut metadata, (1 + area * 257) * 4096)?;
            for record in metadata.as_chunks::<16>().0 {
                let exception = Exception::decode(*record);
                if exception.new_chunk == 0 {
                    source.write_all_at(&output, (1 + kept / 256 * 257) * 4096)?;
                    return Ok(Self {
                        source,
                        kept,
                        chunks: 2 + kept + kept / 256,
                        next: 0,
                        source_chunks: [0; 256],
                        buffer: [0; 4096],
                        position: 4096,
                    });
                }
                source.read_exact_at(&mut block, exception.new_chunk * 4096)?;
                read_origin(exception.old_chunk * 4096, &mut origin)?;
                if block == origin {
                    continue;
                }
                let slot = (kept % 256) as usize * 16;
                output[slot..slot + 16].copy_from_slice(record);
                kept += 1;
                if kept.is_multiple_of(256) {
                    // At most as many exceptions survive as were read. The
                    // destination metadata area is already buffered or behind us.
                    source.write_all_at(&output, (1 + (kept - 1) / 256 * 257) * 4096)?;
                    output.fill(0);
                }
            }
        }
        unreachable!()
    }
    pub(super) fn size_bytes(&self) -> u64 {
        self.chunks * 4096
    }
    fn next_chunk(&mut self) -> io::Result<()> {
        if self.next == 0 {
            self.source.read_exact_at(&mut self.buffer, 0)?;
        } else if (self.next - 1).is_multiple_of(257) {
            let first = (self.next - 1) / 257 * 256;
            let count = (self.kept - first).min(256) as usize;
            self.source
                .read_exact_at(&mut self.buffer, self.next * 4096)?;
            for (slot, record) in self.buffer.as_chunks_mut::<16>().0[..count]
                .iter_mut()
                .enumerate()
            {
                let original = Exception::decode(*record);
                self.source_chunks[slot] = original.new_chunk;
                *record = Exception {
                    old_chunk: original.old_chunk,
                    new_chunk: self.next + 1 + slot as u64,
                }
                .encode();
            }
        } else {
            let slot = ((self.next - 2) % 257) as usize;
            self.source
                .read_exact_at(&mut self.buffer, self.source_chunks[slot] * 4096)?;
        }
        self.next += 1;
        self.position = 0;
        Ok(())
    }
}
impl Read for CompactSnapshot<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.position == self.buffer.len() {
            if self.next == self.chunks {
                return Ok(0);
            }
            self.next_chunk()?;
        }
        let count = bytes.len().min(self.buffer.len() - self.position);
        bytes[..count].copy_from_slice(&self.buffer[self.position..self.position + count]);
        self.position += count;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use devmap_snapshot::Header;

    fn compact(
        source: &mut File,
        mut target: &File,
        origin: impl FnMut(u64, &mut [u8; 4096]) -> io::Result<()>,
    ) -> io::Result<u64> {
        let mut stream = CompactSnapshot::new(source, origin)?;
        io::copy(&mut stream, &mut target)
    }

    fn source(values: &[(u64, u8)]) -> File {
        let file = tempfile::tempfile().unwrap();
        file.set_len((3 + values.len() as u64 + values.len() as u64 / 256) * 4096)
            .unwrap();
        file.write_all_at(
            &Header {
                valid: true,
                chunk_size: Default::default(),
            }
            .encode(),
            0,
        )
        .unwrap();
        for (index, (old, value)) in values.iter().enumerate() {
            let index = index as u64;
            let new_chunk = 2 + index + index / 256;
            file.write_all_at(&[*value; 4096], new_chunk * 4096)
                .unwrap();
            file.write_all_at(
                &Exception {
                    old_chunk: *old,
                    new_chunk,
                }
                .encode(),
                (1 + index / 256 * 257) * 4096 + index % 256 * 16,
            )
            .unwrap();
        }
        file
    }

    fn entries(file: &File) -> Vec<(u64, u8)> {
        let mut result = Vec::new();
        for area in 0.. {
            let mut metadata = [0; 4096];
            file.read_exact_at(&mut metadata, (1 + area * 257) * 4096)
                .unwrap();
            for record in metadata.as_chunks::<16>().0 {
                let entry = Exception::decode(*record);
                if entry.new_chunk == 0 {
                    return result;
                }
                let mut value = [0; 4096];
                file.read_exact_at(&mut value, entry.new_chunk * 4096)
                    .unwrap();
                assert!(value.iter().all(|byte| *byte == value[0]));
                result.push((entry.old_chunk, value[0]));
            }
        }
        unreachable!()
    }

    #[test]
    fn compares_with_parent_including_zero_writes() {
        let mut source = source(&[(0, 0), (1, 9), (2, 0), (3, 7)]);
        let target = tempfile::tempfile().unwrap();
        let size = compact(&mut source, &target, |offset, bytes| {
            bytes.fill(if offset / 4096 == 0 { 0 } else { 9 });
            Ok(())
        })
        .unwrap();
        assert_eq!(entries(&target), [(2, 0), (3, 7)]);
        assert_eq!(entries(&source), [(2, 0), (3, 7)]);
        assert_eq!(size, 4 * 4096);
    }

    #[test]
    fn repacks_across_metadata_boundaries_and_keeps_terminator() {
        for count in [0, 1, 255, 256, 257, 512, 513] {
            let values: Vec<_> = (0..count * 2).map(|i| (i, (i % 2) as u8)).collect();
            let mut source = source(&values);
            let target = tempfile::tempfile().unwrap();
            let size = compact(&mut source, &target, |_, bytes| {
                bytes.fill(0);
                Ok(())
            })
            .unwrap();
            assert_eq!(
                entries(&target),
                values
                    .into_iter()
                    .filter(|(_, value)| *value != 0)
                    .collect::<Vec<_>>()
            );
            assert_eq!(size, (2 + count + count / 256) * 4096);
        }
    }

    #[test]
    fn stream_matches_final_order_at_arbitrary_read_boundaries() {
        let values: Vec<_> = (0..1030).map(|i| (i, (i % 3) as u8)).collect();
        let mut input = source(&values);
        let expected = source(
            &values
                .iter()
                .copied()
                .filter(|(_, value)| *value != 0)
                .collect::<Vec<_>>(),
        );
        for width in [1, 3, 4095, 4096, 8193] {
            let mut stream = CompactSnapshot::new(&mut input, |_, bytes| {
                bytes.fill(0);
                Ok(())
            })
            .unwrap();
            let size = stream.size_bytes();
            let mut actual = Vec::new();
            let mut buffer = vec![0; width];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                if count == 0 {
                    break;
                }
                actual.extend_from_slice(&buffer[..count]);
            }
            let mut reference = vec![0; size as usize];
            expected.read_exact_at(&mut reference, 0).unwrap();
            assert_eq!(actual, reference);
        }
    }

    #[test]
    fn stream_propagates_header_metadata_and_data_read_failures() {
        for truncated in [0, 4096, 8192] {
            let mut input = source(&[(1, 7)]);
            let fault = input.try_clone().unwrap();
            let mut stream = CompactSnapshot::new(&mut input, |_, bytes| {
                bytes.fill(0);
                Ok(())
            })
            .unwrap();
            fault.set_len(truncated).unwrap();
            assert_eq!(stream.read(&mut []).unwrap(), 0);
            assert_eq!(
                io::copy(&mut stream, &mut io::sink()).unwrap_err().kind(),
                io::ErrorKind::UnexpectedEof
            );
        }
    }

    #[test]
    fn origin_and_source_failures_abort_packing() {
        let mut input = source(&[(0, 4)]);
        let output = tempfile::tempfile().unwrap();
        assert_eq!(
            compact(&mut input, &output, |_, _| Err(
                io::ErrorKind::PermissionDenied.into()
            ))
            .unwrap_err()
            .kind(),
            io::ErrorKind::PermissionDenied
        );
        input.set_len(8192).unwrap();
        assert_eq!(
            compact(&mut input, &output, |_, _| Ok(()))
                .unwrap_err()
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
