// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Streaming scute root calculation; no mounts or copies of immutable layer data.
use devmap_core::{BlockSize, Geometry};
use devmap_verity::{Algorithm, Format, HashType, Header, Parameters, Salt};
use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    num::NonZero,
};

#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("scute I/O: {0}")]
    Io(#[from] io::Error),
    #[error("invalid scute: {0}")]
    Invalid(&'static str),
}

/// Describes a known decoded extent. Compression must be measured before this
/// value is constructed; verity never receives an unknown input length.
#[derive(Debug)]
pub struct ScuteExtent {
    size: u64,
    parent: [u8; 32],
    header: Header,
}
impl ScuteExtent {
    pub fn new(size: u64, salt: &[u8], declared_blocks: Option<u64>) -> Result<Self, ChainError> {
        let blocks = size.div_ceil(4096);
        if size == 0 || declared_blocks.is_some_and(|count| count != blocks) {
            return Err(ChainError::Invalid("empty scute or mismatched block count"));
        }
        let parent = salt
            .get(..32)
            .ok_or(ChainError::Invalid("salt lacks parent root"))?
            .try_into()
            .map_err(|_| ChainError::Invalid("invalid parent root"))?;
        let block = BlockSize::from_bytes(NonZero::new(4096).expect("nonzero constant"))
            .expect("valid block size");
        let header = Header {
            uuid: [0; 16],
            parameters: Parameters {
                algorithm: Algorithm::Sha256,
                hash_type: HashType::Normal,
                salt: Salt::new(salt).ok_or(ChainError::Invalid("invalid salt length"))?,
                data: Geometry {
                    size: block,
                    count: NonZero::new(blocks).expect("nonempty extent"),
                },
                hash: block,
            },
        };
        header.parameters.data_size()?;
        Ok(Self {
            size,
            parent,
            header,
        })
    }
    pub fn parent(&self) -> &[u8; 32] {
        &self.parent
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    /// Hash exactly the decoded bytes, with canonical zero block padding.
    /// EOF is mandatory; trailing bytes and late decoder errors fail.
    pub fn root(&self, input: impl Read) -> Result<[u8; 32], ChainError> {
        self.format(input, DiscardTree::default())
    }
    /// Emit the verity header/tree for this exact decoded extent. The caller
    /// must compare the returned root with its trusted root before using it.
    pub fn format(
        &self,
        mut input: impl Read,
        output: impl Write + Seek,
    ) -> Result<[u8; 32], ChainError> {
        let padding = self.header.parameters.data_size()? - self.size;
        // read_exact on the limited extent must fail on a truncated input before
        // padding is allowed to supply any bytes.
        let exact = ExactExtent {
            reader: &mut input,
            remaining: self.size,
        };
        let padded = exact.chain(io::repeat(0).take(padding));
        let root = self.header.format(padded, output)?;
        if input.read(&mut [0])? != 0 {
            return Err(ChainError::Invalid("data exceeds decoded extent"));
        }
        root.as_ref()
            .try_into()
            .map_err(|_| ChainError::Invalid("unexpected hash width"))
    }
}

#[derive(Debug)]
struct ExactExtent<R> {
    reader: R,
    remaining: u64,
}
impl<R: Read> Read for ExactExtent<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() || self.remaining == 0 {
            return Ok(0);
        }
        let limit = self.remaining.min(buffer.len() as u64) as usize;
        let count = self.reader.read(&mut buffer[..limit])?;
        if count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "truncated scute",
            ));
        }
        self.remaining -= count as u64;
        Ok(count)
    }
}

/// The formatter computes roots while emitting a tree. Verification needs only
/// the root, so its seekable output discards bytes instead of allocating a tree.
#[derive(Debug, Default)]
struct DiscardTree {
    position: u64,
    length: u64,
}
impl Write for DiscardTree {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.position = self
            .position
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("tree offset overflow"))?;
        self.length = self.length.max(self.position);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for DiscardTree {
    fn seek(&mut self, seek: SeekFrom) -> io::Result<u64> {
        self.position = match seek {
            SeekFrom::Start(position) => Some(position),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self.length.checked_add_signed(delta),
        }
        .ok_or_else(|| io::Error::other("invalid tree seek"))?;
        Ok(self.position)
    }
}
