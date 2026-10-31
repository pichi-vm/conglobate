// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Cache, CacheError, ResolvedImage};
use crate::artifact::{Digest as ContentDigest, Layer, ScuteDescriptor};
use crate::chain::ScuteExtent;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, BufReader, Read},
};
mod prepared;
pub(crate) use prepared::PreparedScute;

#[derive(Debug)]
pub(crate) struct VerifiedScute {
    digest: ContentDigest,
    root: [u8; 32],
    extent: ScuteExtent,
    compressed: bool,
    blob_size: u64,
}
impl VerifiedScute {
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn digest(&self) -> &ContentDigest {
        &self.digest
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    #[cfg(any(test, feature = "test-fixtures"))]
    pub(crate) fn extent(&self) -> &ScuteExtent {
        &self.extent
    }
    #[cfg(test)]
    pub(crate) fn compressed(&self) -> bool {
        self.compressed
    }
}
/// Content and ancestry verified during these reads. Mount-time dm-verity is
/// still required because the cache itself can change afterward.
#[derive(Debug)]
pub(crate) struct VerifiedChain {
    root: [u8; 32],
    scutes: Vec<VerifiedScute>,
}
impl VerifiedChain {
    pub(super) fn contains_verified(&self, digest: &ContentDigest, size: u64) -> bool {
        self.scutes
            .iter()
            .any(|scute| scute.digest == *digest && scute.blob_size == size)
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn scutes(&self) -> &[VerifiedScute] {
        &self.scutes
    }
}
impl Cache {
    pub(crate) fn verify_chain(
        &self,
        image: &ResolvedImage,
    ) -> Result<Option<VerifiedChain>, CacheError> {
        let manifest = image.manifest();
        let mut layers = manifest
            .layers
            .iter()
            .filter_map(|layer| match layer {
                Layer::Scute(descriptor) => Some((descriptor, false)),
                Layer::ScuteZstd(descriptor) => Some((descriptor, true)),
                _ => None,
            })
            .peekable();
        if layers.peek().is_none() {
            return Ok(None);
        }
        for (key, expected) in [
            ("algo", "sha256"),
            ("data-block-size", "4096"),
            ("hash-block-size", "4096"),
            ("version", "1"),
            ("hash-type", "1"),
        ] {
            if manifest
                .annotations
                .get(&format!("dev.pichi.carapace.verity.{key}"))
                .map(String::as_str)
                != Some(expected)
            {
                return Err(CacheError::Invalid(
                    "unsupported carapace verity parameters",
                ));
            }
        }
        // The manifest declares base-to-tip order. Verify that exact sequence;
        // silently sorting it would accept a different manifest contract.
        let mut root = [0; 32];
        let mut scutes = Vec::new();
        for (descriptor, compressed) in layers {
            let scute = self.verify_scute(descriptor, compressed)?;
            if scute.extent.parent() != &root {
                return Err(CacheError::Invalid("scutes are not in base-to-tip order"));
            }
            root = scute.root;
            scutes.push(scute);
        }
        if manifest.carapace_root()? != Some(root) {
            return Err(CacheError::Invalid(
                "carapace root differs from manifest tip",
            ));
        }
        Ok(Some(VerifiedChain { root, scutes }))
    }
    fn verify_scute(
        &self,
        descriptor: &ScuteDescriptor,
        compressed: bool,
    ) -> Result<VerifiedScute, CacheError> {
        let digest = descriptor
            .digest
            .parse()
            .map_err(|_| CacheError::Invalid("invalid scute digest"))?;
        let salt = hex::decode(&descriptor.annotations.salt)
            .map_err(|_| CacheError::Invalid("invalid scute salt"))?;
        let size = if compressed {
            // No unbounded-length verity input: first measure decoded bytes while
            // authenticating the compressed stream, then re-open and authenticate
            // the hashing pass too. Neither pass copies layer data into scratch.
            let mut source = DecodedBlob::open(self, &digest, descriptor.size, true)?;
            let limit = descriptor
                .annotations
                .data_blocks
                .get()
                .checked_mul(4096)
                .and_then(|v| v.checked_add(1))
                .ok_or(CacheError::Invalid("scute extent overflow"))?;
            let count = io::copy(&mut (&mut source).take(limit), &mut io::sink())?;
            source.finish()?;
            count
        } else {
            descriptor.size
        };
        let extent = ScuteExtent::new(size, &salt, Some(descriptor.annotations.data_blocks.get()))?;
        let mut source = DecodedBlob::open(self, &digest, descriptor.size, compressed)?;
        let root = extent.root(&mut source)?;
        source.finish()?;
        Ok(VerifiedScute {
            digest,
            root,
            extent,
            compressed,
            blob_size: descriptor.size,
        })
    }
}

#[derive(Debug)]
struct CheckedBlob {
    file: File,
    expected: ContentDigest,
    size: u64,
    count: u64,
    hash: Sha256,
}
impl CheckedBlob {
    fn open(cache: &Cache, digest: &ContentDigest, size: u64) -> Result<Self, CacheError> {
        let file = cache.blob(digest)?;
        if file.metadata()?.len() != size {
            return Err(CacheError::Invalid("scute blob size mismatch"));
        }
        Ok(Self {
            file,
            expected: digest.clone(),
            size,
            count: 0,
            hash: Sha256::new(),
        })
    }
    fn finish(mut self) -> Result<(), CacheError> {
        if self.read(&mut [0])? != 0
            || self.count != self.size
            || self.hash.finalize().as_slice() != self.expected.as_bytes()
        {
            return Err(CacheError::Invalid("scute blob content mismatch"));
        }
        Ok(())
    }
}
impl Read for CheckedBlob {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.file.read(bytes)?;
        self.count = self
            .count
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("blob size overflow"))?;
        if self.count > self.size {
            return Err(io::Error::other("blob grew during read"));
        }
        self.hash.update(&bytes[..count]);
        Ok(count)
    }
}

enum DecodedBlob {
    Plain(CheckedBlob),
    Compressed(zstd::stream::read::Decoder<'static, BufReader<CheckedBlob>>),
}
impl DecodedBlob {
    fn open(
        cache: &Cache,
        digest: &ContentDigest,
        size: u64,
        compressed: bool,
    ) -> Result<Self, CacheError> {
        let source = CheckedBlob::open(cache, digest, size)?;
        Ok(if compressed {
            Self::Compressed(zstd::stream::read::Decoder::new(source)?)
        } else {
            Self::Plain(source)
        })
    }
    fn finish(mut self) -> Result<(), CacheError> {
        if self.read(&mut [0])? != 0 {
            return Err(CacheError::Invalid("unconsumed decoded data"));
        }
        match self {
            Self::Plain(source) => source.finish(),
            Self::Compressed(decoder) => {
                let source = decoder.finish();
                if !source.buffer().is_empty() {
                    return Err(CacheError::Invalid("unconsumed compressed data"));
                }
                source.into_inner().finish()
            }
        }
    }
}
impl Read for DecodedBlob {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Plain(source) => source.read(bytes),
            Self::Compressed(source) => source.read(bytes),
        }
    }
}
