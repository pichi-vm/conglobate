// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Read-only content access. A receipt verifies a read, never future cache bytes.
mod chain;
pub(crate) use chain::{PreparedScute, VerifiedChain};
mod resolve;
pub(crate) use resolve::{HostArchitecture, ResolvedImage};

use crate::artifact::Digest as ContentDigest;
use rustix::fs::{Mode, OFlags, ResolveFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum CacheError {
    #[error(transparent)]
    Chain(#[from] crate::chain::ChainError),
    #[error("invalid OCI JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("invalid artifact manifest: {0}")]
    Manifest(#[from] crate::artifact::Error),
    #[error("cache I/O: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Firmware(#[from] crate::firmware::FirmwareError),
    #[error("invalid cache content: {0}")]
    Invalid(&'static str),
}

/// JSON indexes, manifests and DTBs. Layers and firmware use other readers.
const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;

/// Pins directory authority; no method creates, downloads or modifies content.
#[derive(Debug)]
pub(crate) struct Cache {
    directory: File,
}
impl Cache {
    /// Open existing directory authority without fetching or validating blobs.
    /// Individual reads and chain verification establish content identities.
    ///
    /// # Errors
    /// Returns an I/O error if the supplied directory cannot be opened.
    pub(crate) fn open(path: &Path) -> Result<Self, CacheError> {
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
    ///
    /// When `expected_size` is set it must equal the file length, and the
    /// bytes must hash to `digest`. A descriptor that is only a path key uses
    /// `metadata_by_path`.
    pub(crate) fn metadata(
        &self,
        digest: &ContentDigest,
        expected_size: Option<u64>,
    ) -> Result<VerifiedMetadata, CacheError> {
        let bytes = self.read_metadata(digest, expected_size)?;
        if ContentDigest::Sha256(Sha256::digest(&bytes).into()) != *digest {
            return Err(CacheError::Invalid("metadata content mismatch"));
        }
        Ok(VerifiedMetadata {
            digest: digest.clone(),
            bytes,
        })
    }
    /// Read a small blob whose path digest is not a content commitment.
    ///
    /// The receipt digest is the SHA-256 of the captured bytes. The metadata
    /// size cap still applies. Callers authenticate every blob the captured
    /// document names; this method does not.
    fn metadata_by_path(
        &self,
        path_digest: &ContentDigest,
    ) -> Result<VerifiedMetadata, CacheError> {
        let bytes = self.read_metadata(path_digest, None)?;
        Ok(VerifiedMetadata {
            digest: ContentDigest::Sha256(Sha256::digest(&bytes).into()),
            bytes,
        })
    }
    fn read_metadata(
        &self,
        digest: &ContentDigest,
        expected_size: Option<u64>,
    ) -> Result<Vec<u8>, CacheError> {
        let mut file = self.blob(digest)?;
        let size = file.metadata()?.len();
        if size > MAX_METADATA_BYTES || expected_size.is_some_and(|expected| expected != size) {
            return Err(CacheError::Invalid(
                "metadata size mismatch or limit exceeded",
            ));
        }
        let mut bytes = Vec::new();
        (&mut file).take(size + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != size {
            return Err(CacheError::Invalid("metadata content mismatch"));
        }
        Ok(bytes)
    }
    /// Capture a firmware blob after authenticating its declared size and digest.
    /// Firmware is not subject to the small JSON/DTB metadata size limit.
    pub(crate) fn firmware_contract(
        &self,
        digest: &ContentDigest,
        size: u64,
    ) -> Result<crate::firmware::DtbContract, CacheError> {
        let size_usize = usize::try_from(size)
            .map_err(|_| CacheError::Invalid("firmware size exceeds address space"))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size_usize)
            .map_err(|error| io::Error::other(error.to_string()))?;
        self.copy_verified(digest, size, &mut bytes)?;
        Ok(crate::firmware::DtbContract::from_pmi(&bytes)?)
    }
    /// Verify one complete read without retaining an unverified future read handle.
    /// Mounting layers requires independent dm-verity verification at use time.
    pub(crate) fn verify(&self, digest: &ContentDigest, size: u64) -> Result<(), CacheError> {
        self.copy_verified(digest, size, &mut io::sink())
    }
    /// Copy and verify the same byte stream. On error the destination may hold
    /// partial or invalid bytes; callers must keep it private and discard it.
    /// A prior receipt never authorizes a later unverified cache read.
    pub(crate) fn copy_verified(
        &self,
        digest: &ContentDigest,
        size: u64,
        destination: &mut impl Write,
    ) -> Result<(), CacheError> {
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
            destination.write_all(&buffer[..count])?;
            hash.update(&buffer[..count]);
            remaining -= count as u64;
        }
        if file.read(&mut buffer[..1])? != 0 || hash.finalize().as_slice() != digest.as_bytes() {
            return Err(CacheError::Invalid("blob content mismatch"));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct VerifiedMetadata {
    digest: ContentDigest,
    bytes: Vec<u8>,
}
impl VerifiedMetadata {
    pub(crate) fn digest(&self) -> &ContentDigest {
        &self.digest
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
