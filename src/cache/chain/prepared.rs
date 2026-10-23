// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Cache, CacheError, DecodedBlob, VerifiedScute};
use crate::storage::{Scratch, Workspace};
use std::{
    fs::File,
    io::{self, Read, Seek},
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
            let limit = self
                .extent
                .size()
                .checked_add(1)
                .ok_or(CacheError::Invalid("decoded extent overflow"))?;
            if io::copy(&mut (&mut source).take(limit), &mut data)? != self.extent.size() {
                return Err(CacheError::Invalid("decoded scute size changed"));
            }
            source.finish()?;
            data.rewind()?;
            if self.extent.format(&mut data, &mut hash)? != self.root {
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
