// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Cache, CacheError, DecodedBlob, VerifiedScute};
use crate::storage::{Scratch, Workspace};
use std::{
    fs::File,
    io::{self, Read, Write},
};

/// Owns data and generated verification metadata for future dm-verity assembly.
/// No writable handles escape. A successful preparation authenticates this read;
/// cache-backed bytes still require dm-verity on every subsequent kernel read.
#[derive(Debug)]
pub struct PreparedScute<'scute, 'workspace> {
    source: Source<'scute, 'workspace>,
}

#[derive(Debug)]
enum Source<'s, 'w> {
    Cached {
        scute: &'s VerifiedScute,
        data: Data<'w>,
        hash: Scratch<'w>,
    },
    Produced {
        scute: std::sync::Arc<crate::storage::SealedScute<'w>>,
        header: Box<devmap_verity::Header>,
    },
}

#[derive(Debug)]
enum Data<'workspace> {
    Cache(File),
    Materialized(Scratch<'workspace>),
}

impl<'s> VerifiedScute {
    pub fn prepare<'w>(
        &'s self,
        cache: &Cache,
        workspace: &'w Workspace,
    ) -> Result<PreparedScute<'s, 'w>, CacheError> {
        let mut hash = workspace.scratch()?;
        let data = if !self.compressed && self.extent.size().is_multiple_of(4096) {
            // Retain the same open inode used to generate the tree. Later path
            // replacement cannot redirect this owner, and later content changes
            // must be checked by dm-verity against the trusted root.
            let mut data = cache.blob(&self.digest)?;
            if data.metadata()?.len() != self.blob_size {
                return Err(CacheError::Invalid("scute blob size changed"));
            }
            if self.extent.format(&mut data, &mut hash)? != self.root {
                return Err(CacheError::Invalid("prepared scute root mismatch"));
            }
            Data::Cache(data)
        } else {
            // Compression and a partial final block require materialization;
            // never use this path to copy ordinary aligned cache-backed data.
            let mut source =
                DecodedBlob::open(cache, &self.digest, self.blob_size, self.compressed)?;
            let mut data = workspace.scratch()?;
            // Capture only decoded bytes while the formatter authenticates the
            // exact extent. Its synthetic final-block padding is not captured.
            let root = self.extent.format(
                MaterializingReader {
                    source: &mut source,
                    output: &mut data,
                },
                &mut hash,
            )?;
            // Also authenticate the encoded blob and force decoder completion;
            // a matching verity root alone is not an OCI digest receipt.
            source.finish()?;
            if root != self.root {
                return Err(CacheError::Invalid("prepared scute root mismatch"));
            }
            let padded = self
                .extent
                .size()
                .div_ceil(4096)
                .checked_mul(4096)
                .ok_or(CacheError::Invalid("padded extent overflow"))?;
            data.set_len(padded)?;
            data.sync_all()?;
            Data::Materialized(data)
        };
        hash.sync_all()?;
        Ok(PreparedScute {
            source: Source::Cached {
                scute: self,
                data,
                hash,
            },
        })
    }
}

impl<'s, 'w> PreparedScute<'s, 'w> {
    pub(crate) fn produced(
        scute: std::sync::Arc<crate::storage::SealedScute<'w>>,
    ) -> Result<Self, crate::chain::ChainError> {
        let header = crate::chain::ScuteExtent::new(
            scute.descriptor().size,
            scute.parent(),
            scute.descriptor().annotations.data_blocks,
        )?
        .header();
        Ok(Self {
            source: Source::Produced {
                scute,
                header: Box::new(header),
            },
        })
    }
    pub(crate) fn files(&self) -> (&File, &File) {
        match &self.source {
            Source::Cached { data, hash, .. } => {
                let data = match data {
                    Data::Cache(file) => file,
                    Data::Materialized(scratch) => scratch.file(),
                };
                (data, hash.file())
            }
            Source::Produced { scute, .. } => scute.files(),
        }
    }
    pub(crate) fn header(&self) -> devmap_verity::Header {
        match &self.source {
            Source::Cached { scute, .. } => scute.extent.header(),
            Source::Produced { header, .. } => **header,
        }
    }
    pub fn root(&self) -> &[u8; 32] {
        match &self.source {
            Source::Cached { scute, .. } => scute.root(),
            Source::Produced { scute, .. } => scute.root(),
        }
    }
    pub fn is_cache_backed(&self) -> bool {
        matches!(
            &self.source,
            Source::Cached {
                data: Data::Cache(_),
                ..
            }
        )
    }
    pub fn data_size(&self) -> io::Result<u64> {
        Ok(self.files().0.metadata()?.len())
    }
    pub fn hash_size(&self) -> io::Result<u64> {
        Ok(self.files().1.metadata()?.len())
    }
}

/// Couples decoded consumption to materialization without a second data read.
/// A failed write fails the read too; no successful preparation can omit bytes.
struct MaterializingReader<R, W> {
    source: R,
    output: W,
}
impl<R: Read, W: Write> Read for MaterializingReader<R, W> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.source.read(bytes)?;
        self.output.write_all(&bytes[..count])?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chain::ScuteExtent;
    use std::io::Cursor;

    #[test]
    fn materialization_preserves_exact_bytes_and_canonical_root() {
        for size in [1, 4095, 4096, 4097] {
            let bytes = vec![0x5a; size];
            let extent = ScuteExtent::new(size as u64, &[0; 32], None).unwrap();
            let mut output = Vec::new();
            let actual = extent
                .format(
                    MaterializingReader {
                        source: bytes.as_slice(),
                        output: &mut output,
                    },
                    Cursor::new(Vec::new()),
                )
                .unwrap();
            assert_eq!(actual, extent.root(bytes.as_slice()).unwrap());
            assert_eq!(output, bytes); // no synthetic padding captured
        }
    }

    #[test]
    fn materialization_rejects_short_long_and_late_failed_sources() {
        let extent = ScuteExtent::new(4096, &[0; 32], None).unwrap();
        for size in [4095, 4097] {
            let bytes = vec![0x5a; size];
            assert!(
                extent
                    .root(MaterializingReader {
                        source: bytes.as_slice(),
                        output: Vec::new(),
                    })
                    .is_err()
            );
        }
        struct LateFailure;
        impl Read for LateFailure {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::from_raw_os_error(5))
            }
        }
        let bytes = [0x5a; 4096];
        let failure = extent
            .root(MaterializingReader {
                source: bytes.as_slice().chain(LateFailure),
                output: Vec::new(),
            })
            .unwrap_err();
        assert!(matches!(failure, crate::chain::ChainError::Io(e) if e.raw_os_error() == Some(5)));
    }

    #[test]
    fn materialization_propagates_output_failure() {
        // A fixed-size slice cannot accept the complete decoded extent.
        let mut output = [0; 1];
        let error = ScuteExtent::new(4096, &[0; 32], None)
            .unwrap()
            .root(MaterializingReader {
                source: &[0x5a; 4096][..],
                output: &mut output[..],
            })
            .unwrap_err();
        assert!(
            matches!(error, crate::chain::ChainError::Io(e) if e.kind() == io::ErrorKind::WriteZero)
        );
    }
}
