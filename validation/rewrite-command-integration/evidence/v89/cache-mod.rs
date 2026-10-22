// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Read-only content access. A receipt verifies a read, never future cache bytes.
mod chain;
pub use chain::{PreparedScute, VerifiedChain, VerifiedScute};
mod resolve;
pub use resolve::{HostArchitecture, ResolvedImage};

use pichi_artifact::Digest as ContentDigest;
use rustix::fs::{Mode, OFlags, ResolveFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error(transparent)]
    Chain(#[from] crate::chain::ChainError),
    #[error("invalid OCI JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid artifact manifest: {0}")]
    Manifest(String),
    #[error("cache I/O: {0}")]
    Io(#[from] io::Error),
    #[error("invalid cache content: {0}")]
    Invalid(&'static str),
}

/// Pins directory authority; no method creates, downloads or modifies content.
#[derive(Debug)]
pub struct Cache {
    directory: File,
}
impl Cache {
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        let fd = rustix::fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(io::Error::from)?;
        Ok(Self {
            directory: fd.into(),
        })
    }
    fn blob(&self, digest: &ContentDigest) -> Result<File, CacheError> {
        let path = format!("blobs/{}/{}", digest.algo(), digest.hex());
        let fd = rustix::fs::openat2(
            &self.directory,
            path,
            OFlags::RDONLY | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS,
        )
        .map_err(io::Error::from)?;
        let file = File::from(fd);
        if !file.metadata()?.is_file() {
            return Err(CacheError::Invalid("blob is not a regular file"));
        }
        Ok(file)
    }
    /// Capture small metadata exactly once. Large immutable layers stay cache-backed.
    pub fn metadata(
        &self,
        digest: &ContentDigest,
        expected_size: Option<u64>,
    ) -> Result<VerifiedMetadata, CacheError> {
        const MAX_METADATA: u64 = 16 * 1024 * 1024;
        let mut file = self.blob(digest)?;
        let size = file.metadata()?.len();
        if size > MAX_METADATA || expected_size.is_some_and(|expected| expected != size) {
            return Err(CacheError::Invalid(
                "metadata size mismatch or limit exceeded",
            ));
        }
        let mut bytes = Vec::new();
        (&mut file).take(size + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != size || ContentDigest::from_bytes_sha256(&bytes) != *digest {
            return Err(CacheError::Invalid("metadata content mismatch"));
        }
        Ok(VerifiedMetadata {
            digest: digest.clone(),
            bytes,
        })
    }
    /// Verify one complete read without retaining an unverified future read handle.
    /// Mounting layers requires independent dm-verity verification at use time.
    pub fn verify(&self, digest: &ContentDigest, size: u64) -> Result<BlobReceipt, CacheError> {
        let mut file = self.blob(digest)?;
        if file.metadata()?.len() != size {
            return Err(CacheError::Invalid("blob size mismatch"));
        }
        let mut hash = Sha256::new();
        let mut remaining = size;
        let mut buffer = [0; 64 * 1024];
        while remaining > 0 {
            let capacity = remaining.min(buffer.len() as u64) as usize;
            let count = file.read(&mut buffer[..capacity])?;
            if count == 0 {
                return Err(CacheError::Invalid("blob truncated during read"));
            }
            hash.update(&buffer[..count]);
            remaining -= count as u64;
        }
        if file.read(&mut buffer[..1])? != 0 || hash.finalize().as_slice() != digest.as_bytes() {
            return Err(CacheError::Invalid("blob content mismatch"));
        }
        Ok(BlobReceipt {
            digest: digest.clone(),
            size,
        })
    }
}

#[derive(Debug)]
pub struct VerifiedMetadata {
    digest: ContentDigest,
    bytes: Vec<u8>,
}
impl VerifiedMetadata {
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
#[derive(Debug)]
pub struct BlobReceipt {
    digest: ContentDigest,
    size: u64,
}
impl BlobReceipt {
    pub fn digest(&self) -> &ContentDigest {
        &self.digest
    }
    pub fn size(&self) -> u64 {
        self.size
    }
}
