// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use devmap_snapshot::Exception;
use std::{fs::File, io, os::unix::fs::FileExt};

/// Repack an already validated, detached 4KiB persistent snapshot. A separate
/// destination prevents packed writes from destroying unread source blocks.
/// The origin reader must describe the same immutable chain used by the writer.
pub(super) fn compact(
    source: &File,
    target: &File,
    mut read_origin: impl FnMut(u64, &mut [u8; 4096]) -> io::Result<()>,
) -> io::Result<u64> {
    let mut block = [0; 4096];
    source.read_exact_at(&mut block, 0)?;
    target.write_all_at(&block, 0)?;
    let mut origin = [0; 4096];
    let mut output = [0; 4096];
    let mut kept = 0u64;
    for area in 0.. {
        let mut metadata = [0; 4096];
        source.read_exact_at(&mut metadata, (1 + area * 257) * 4096)?;
        for record in metadata.as_chunks::<16>().0 {
            let exception = Exception::decode(*record);
            if exception.new_chunk == 0 {
                let metadata_chunk = 1 + (kept / 256) * 257;
                target.write_all_at(&output, metadata_chunk * 4096)?;
                let chunks = if kept.is_multiple_of(256) {
                    metadata_chunk + 1
                } else {
                    2 + kept + (kept - 1) / 256
                };
                let bytes = chunks * 4096;
                target.set_len(bytes)?;
                return Ok(bytes);
            }
            source.read_exact_at(&mut block, exception.new_chunk * 4096)?;
            read_origin(exception.old_chunk * 4096, &mut origin)?;
            if block == origin {
                continue;
            }
            let new_chunk = 2 + kept + kept / 256;
            target.write_all_at(&block, new_chunk * 4096)?;
            let slot = (kept % 256) as usize * 16;
            output[slot..slot + 16].copy_from_slice(
                &Exception {
                    old_chunk: exception.old_chunk,
                    new_chunk,
                }
                .encode(),
            );
            kept += 1;
            if kept.is_multiple_of(256) {
                target.write_all_at(&output, (1 + ((kept - 1) / 256) * 257) * 4096)?;
                output.fill(0);
            }
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use devmap_snapshot::Header;

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
        let source = source(&[(0, 0), (1, 9), (2, 0), (3, 7)]);
        let target = tempfile::tempfile().unwrap();
        let size = compact(&source, &target, |offset, bytes| {
            bytes.fill(if offset / 4096 == 0 { 0 } else { 9 });
            Ok(())
        })
        .unwrap();
        assert_eq!(entries(&target), [(2, 0), (3, 7)]);
        assert_eq!(entries(&source), [(0, 0), (1, 9), (2, 0), (3, 7)]);
        assert_eq!(size, 4 * 4096);
    }

    #[test]
    fn repacks_across_metadata_boundaries_and_keeps_terminator() {
        for count in [0, 1, 255, 256, 257, 512, 513] {
            let values: Vec<_> = (0..count * 2).map(|i| (i, (i % 2) as u8)).collect();
            let source = source(&values);
            let target = tempfile::tempfile().unwrap();
            let size = compact(&source, &target, |_, bytes| {
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
    fn origin_and_source_failures_abort_packing() {
        let input = source(&[(0, 4)]);
        let output = tempfile::tempfile().unwrap();
        assert_eq!(
            compact(&input, &output, |_, _| Err(
                io::ErrorKind::PermissionDenied.into()
            ))
            .unwrap_err()
            .kind(),
            io::ErrorKind::PermissionDenied
        );
        input.set_len(8192).unwrap();
        assert_eq!(
            compact(&input, &output, |_, _| Ok(())).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
}
