// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Private output storage and atomic filesystem publication. These low-level
//! owners do not prove a successful build or validate inventory/manifest relations.
mod assembly;
pub(crate) use assembly::{AssembledImage, AssemblyFailure};

use crate::cache::{BlobReceipt, Cache, CacheError};
use pichi_artifact::Digest;
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Read, Write},
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::{Path, PathBuf},
};

/// Owns a temporary sibling of the requested destination, anchored to its
/// parent descriptor. No unverified cache stream reaches the published name.
/// The build coordinator must validate the complete layout before publication.
#[derive(Debug)]
pub(crate) struct StagedDirectory {
    temporary: tempfile::TempDir,
    parent: File,
    destination: OsString,
}
#[derive(Debug, thiserror::Error)]
#[error("output publication: {source}")]
pub(crate) struct PublicationFailure {
    source: io::Error,
    staged: StagedDirectory,
}
impl PublicationFailure {
    pub(crate) fn into_parts(self) -> (io::Error, StagedDirectory) {
        (self.source, self.staged)
    }
}
impl StagedDirectory {
    /// Create private staging beside a new destination, anchored to its parent.
    /// This does not validate an image or publish the destination.
    ///
    /// # Errors
    /// Rejects a nameless/existing destination, an inaccessible or non-directory
    /// parent, and failures to create private sibling storage.
    pub(crate) fn create(destination: &Path) -> io::Result<Self> {
        let name = destination.file_name().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "output must name a new directory",
            )
        })?;
        let parent_path = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = File::open(parent_path)?;
        if !parent.metadata()?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "output parent",
            ));
        }
        match rustix::fs::statat(&parent, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => {}
            Err(error) => return Err(error.into()),
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "output exists",
                ));
            }
        }
        let temporary = tempfile::Builder::new()
            .prefix(".conglobate-output-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))?;
        fs::create_dir_all(temporary.path().join("blobs/sha256"))?;
        Ok(Self {
            temporary,
            parent,
            destination: name.to_owned(),
        })
    }
    fn blob_path(&self, digest: &Digest) -> PathBuf {
        self.temporary
            .path()
            .join("blobs")
            .join(digest.algo())
            .join(digest.hex())
    }
    /// Copy the exact stream being verified to a private temporary file. Failed
    /// verification or writes remove that file, leaving no digest-named blob.
    pub(crate) fn copy_blob(
        &self,
        cache: &Cache,
        digest: &Digest,
        size: u64,
    ) -> Result<BlobReceipt, CacheError> {
        let target = self.blob_path(digest);
        let mut pending = tempfile::NamedTempFile::new_in(self.temporary.path())?;
        let receipt = cache.copy_verified(digest, size, pending.as_file_mut())?;
        pending.as_file().sync_all()?;
        // The directory is supervisor-private; replacing the same digest here
        // replaces only already-verified staging data, never published content.
        pending.persist(target).map_err(|error| error.error)?;
        Ok(receipt)
    }
    /// Stream a sealed layer without buffering its contents in memory. Verify
    /// the exact written bytes before giving them their digest name.
    pub(crate) fn store_scute(
        &self,
        scute: &crate::storage::SealedScute<'_>,
    ) -> Result<(), CacheError> {
        let digest = scute
            .descriptor()
            .digest
            .parse()
            .map_err(|_| CacheError::Invalid("invalid produced scute digest"))?;
        self.store_verified(scute.data_reader(), &digest, scute.descriptor().size)
    }
    fn store_verified(
        &self,
        mut reader: impl Read,
        digest: &Digest,
        size: u64,
    ) -> Result<(), CacheError> {
        use sha2::{Digest as _, Sha256};
        let mut pending = tempfile::NamedTempFile::new_in(self.temporary.path())?;
        let mut hash = Sha256::new();
        let mut remaining = size;
        let mut buffer = [0u8; 65536];
        while remaining != 0 {
            let count = remaining.min(buffer.len() as u64) as usize;
            let count = reader.read(&mut buffer[..count])?;
            if count == 0 {
                return Err(CacheError::Invalid("truncated produced scute"));
            }
            pending.write_all(&buffer[..count])?;
            hash.update(&buffer[..count]);
            remaining -= count as u64;
        }
        if reader.read(&mut buffer[..1])? != 0 || hash.finalize().as_slice() != digest.as_bytes() {
            return Err(CacheError::Invalid("produced scute content mismatch"));
        }
        pending.as_file().sync_all()?;
        pending
            .persist(self.blob_path(digest))
            .map_err(|error| error.error)?;
        Ok(())
    }
    /// Store captured or newly produced small content, deriving its identity
    /// from these bytes. The coordinator supplies the matching descriptor.
    pub(crate) fn store_blob(&self, bytes: &[u8]) -> io::Result<Digest> {
        let digest = Digest::from_bytes_sha256(bytes);
        self.write_file(&self.blob_path(&digest), bytes)?;
        Ok(digest)
    }
    fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut pending = tempfile::NamedTempFile::new_in(self.temporary.path())?;
        pending.write_all(bytes)?;
        pending.as_file().sync_all()?;
        pending.persist(path).map_err(|error| error.error)?;
        Ok(())
    }
    /// Add inventory without rewriting the already assembled image layout.
    /// Serialization and structural validation belong to the coordinator.
    pub(crate) fn write_inventory(&self, inventory: &[u8]) -> io::Result<()> {
        self.write_file(&self.temporary.path().join("inventory.cbor"), inventory)
    }
    /// Atomic no-replacement filesystem operation, not a build-success receipt.
    /// Failure retains staging for explicit cleanup. No fallible operation is
    /// performed after rename, so Err never means the destination was published.
    pub(crate) fn publish(self) -> Result<(), PublicationFailure> {
        let result = rustix::fs::renameat_with(
            &self.parent,
            self.temporary
                .path()
                .file_name()
                .expect("generated staging name"),
            &self.parent,
            &self.destination,
            rustix::fs::RenameFlags::NOREPLACE,
        );
        match result {
            Ok(()) => {
                let _ = self.temporary.keep();
                Ok(())
            }
            Err(error) => Err(PublicationFailure {
                source: error.into(),
                staged: self,
            }),
        }
    }
    pub(crate) fn discard(self) -> io::Result<()> {
        self.temporary.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn produced_stream_must_match_size_and_digest_before_installation() {
        let parent = tempfile::tempdir().unwrap();
        let staged = StagedDirectory::create(&parent.path().join("output")).unwrap();
        let bytes = vec![37u8; 150_000];
        let digest = Digest::from_bytes_sha256(&bytes);
        for (input, size) in [
            (&bytes[..bytes.len() - 1], bytes.len() as u64),
            (&bytes[..], bytes.len() as u64 - 1),
            (&bytes[..], bytes.len() as u64 + 1),
        ] {
            assert!(staged.store_verified(input, &digest, size).is_err());
            assert!(!staged.blob_path(&digest).exists());
        }
        let wrong = Digest::from_bytes_sha256(b"wrong");
        assert!(
            staged
                .store_verified(&bytes[..], &wrong, bytes.len() as u64)
                .is_err()
        );
        assert!(!staged.blob_path(&wrong).exists());
        staged
            .store_verified(&bytes[..], &digest, bytes.len() as u64)
            .unwrap();
        assert_eq!(fs::read(staged.blob_path(&digest)).unwrap(), bytes);
        assert_eq!(fs::read_dir(staged.temporary.path()).unwrap().count(), 1);
        staged.discard().unwrap();
    }
    #[test]
    fn late_collision_never_replaces_existing_output_and_retains_staging() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("output");
        let staged = StagedDirectory::create(&destination).unwrap();
        staged.store_blob(b"payload").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("sentinel"), b"existing").unwrap();
        let (error, staged) = staged.publish().unwrap_err().into_parts();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(destination.join("sentinel")).unwrap(), b"existing");
        staged.discard().unwrap();
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
    }
    #[test]
    fn cache_mutation_does_not_install_invalid_blob() {
        let parent = tempfile::tempdir().unwrap();
        let cache_path = parent.path().join("cache");
        fs::create_dir_all(cache_path.join("blobs/sha256")).unwrap();
        let digest = Digest::from_bytes_sha256(b"original");
        let blob = cache_path.join("blobs/sha256").join(digest.hex());
        fs::write(&blob, b"original").unwrap();
        let cache = Cache::open(&cache_path).unwrap();
        cache.verify(&digest, 8).unwrap();
        fs::write(&blob, b"modified").unwrap();
        let staged = StagedDirectory::create(&parent.path().join("output")).unwrap();
        assert!(staged.copy_blob(&cache, &digest, 8).is_err());
        assert!(!staged.blob_path(&digest).exists());
        assert_eq!(fs::read_dir(staged.temporary.path()).unwrap().count(), 1);
        fs::write(&blob, b"original").unwrap();
        staged.copy_blob(&cache, &digest, 8).unwrap();
        assert_eq!(fs::read(staged.blob_path(&digest)).unwrap(), b"original");
    }
    #[test]
    fn publication_uses_pinned_parent_and_includes_exact_bytes() {
        let root = tempfile::tempdir().unwrap();
        let parent = root.path().join("parent");
        fs::create_dir(&parent).unwrap();
        let staged = StagedDirectory::create(&parent.join("output")).unwrap();
        let digest = staged.store_blob(b"{}").unwrap();
        staged.write_inventory(b"inventory").unwrap();
        let moved = root.path().join("moved");
        fs::rename(&parent, &moved).unwrap();
        fs::create_dir(&parent).unwrap();
        staged.publish().unwrap();
        assert!(!parent.join("output").exists());
        assert_eq!(
            fs::read(moved.join("output/inventory.cbor")).unwrap(),
            b"inventory"
        );
        assert_eq!(
            fs::read(moved.join("output/blobs/sha256").join(digest.hex())).unwrap(),
            b"{}"
        );
        assert!(StagedDirectory::create(&moved.join("output")).is_err());
    }
}
