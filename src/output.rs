// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

mod assembly;
pub(crate) use assembly::AssembledImage;

use crate::artifact::Digest;
use crate::cache::{Cache, CacheError};
use sha2::{Digest as _, Sha256};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Read, Write},
    os::{
        fd::AsRawFd,
        unix::fs::{OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

/// How staged content is published onto the destination path.
#[derive(Debug)]
enum PublishKind {
    /// Destination must not exist; rename the staging directory to `name`.
    RenameDirectory { name: OsString },
    /// Destination exists and was empty; move staging children into it.
    MoveChildren,
}

/// Owns private staging for an output destination. No unverified cache stream
/// reaches a published name. The coordinator must validate the layout before
/// publication.
#[derive(Debug)]
pub(crate) struct StagedDirectory {
    temporary: tempfile::TempDir,
    parent: File,
    kind: PublishKind,
}
impl StagedDirectory {
    /// Create private staging for `destination`.
    ///
    /// - If `destination` does not exist: stage as a sibling under its parent
    ///   (CLI `--output` naming a new directory).
    /// - If `destination` exists as an empty directory: stage inside it and
    ///   publish layout entries into that directory (appliance empty virtiofs
    ///   mount at `/run/pichi/output`).
    ///
    /// # Errors
    /// Rejects a nameless destination, a non-empty or non-directory existing
    /// path, an inaccessible parent, and staging-directory creation failures.
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
        let parent_dir = File::open(parent_path)?;
        if !parent_dir.metadata()?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "output parent",
            ));
        }
        match rustix::fs::statat(&parent_dir, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW) {
            Err(rustix::io::Errno::NOENT) => {
                let temporary = Self::new_staging(&parent_dir)?;
                Ok(Self {
                    temporary,
                    parent: parent_dir,
                    kind: PublishKind::RenameDirectory {
                        name: name.to_owned(),
                    },
                })
            }
            Err(error) => Err(error.into()),
            Ok(_) => {
                let meta = fs::symlink_metadata(destination)?;
                if !meta.is_dir() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "output exists",
                    ));
                }
                let mut entries = fs::read_dir(destination)?;
                if entries.next().transpose()?.is_some() {
                    return Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "output exists",
                    ));
                }
                let mount = File::open(destination)?;
                let temporary = Self::new_staging(&mount)?;
                Ok(Self {
                    temporary,
                    parent: mount,
                    kind: PublishKind::MoveChildren,
                })
            }
        }
    }

    fn new_staging(dir: &File) -> io::Result<tempfile::TempDir> {
        let temporary = tempfile::Builder::new()
            .prefix(".conglobate-output-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", dir.as_raw_fd()))?;
        fs::create_dir_all(temporary.path().join("blobs/sha256"))?;
        Ok(temporary)
    }

    fn blob_path(&self, digest: &Digest) -> PathBuf {
        self.temporary
            .path()
            .join("blobs")
            .join(digest.algo())
            .join(digest.hex())
    }
    /// Write at the final private staging name and remove it on failure.
    /// Keeping the file must not rename it: the current virtiofs transport
    /// invalidates renamed inode IDs, so validation cannot reopen those files.
    /// Only publication exposes these names outside the supervisor's staging.
    fn pending_file(&self, path: &Path) -> io::Result<tempfile::NamedTempFile> {
        let path = tempfile::TempPath::try_from_path(path)?;
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        Ok(tempfile::NamedTempFile::from_parts(file, path))
    }
    /// Copy the exact stream being verified to private staging. Failed
    /// verification or writes remove it, leaving no digest-named blob.
    pub(crate) fn copy_blob(
        &self,
        cache: &Cache,
        digest: &Digest,
        size: u64,
    ) -> Result<(), CacheError> {
        let target = self.blob_path(digest);
        let mut pending = self.pending_file(&target)?;
        cache.copy_verified(digest, size, pending.as_file_mut())?;
        pending.as_file().sync_all()?;
        pending.keep().map_err(|error| error.error)?;
        Ok(())
    }
    /// Stream a sealed layer without buffering its contents in memory. Verify
    /// the exact written bytes before retaining their private staging name.
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
    pub(crate) fn store_verified(
        &self,
        mut reader: impl Read,
        digest: &Digest,
        size: u64,
    ) -> Result<(), CacheError> {
        let mut pending = self.pending_file(&self.blob_path(digest))?;
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
        pending.keep().map_err(|error| error.error)?;
        Ok(())
    }
    /// Store captured or newly produced small content, deriving its identity
    /// from these bytes. The coordinator supplies the matching descriptor.
    pub(crate) fn store_blob(&self, bytes: &[u8]) -> io::Result<Digest> {
        let digest = Digest::Sha256(Sha256::digest(bytes).into());
        self.write_file(&self.blob_path(&digest), bytes)?;
        Ok(digest)
    }
    fn write_file(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut pending = self.pending_file(path)?;
        pending.write_all(bytes)?;
        pending.as_file().sync_all()?;
        pending.keep().map_err(|error| error.error)?;
        Ok(())
    }
    /// Publish staged layout to the destination. Not a build-success receipt.
    ///
    /// New-directory mode uses one atomic no-replace rename of the staging
    /// directory. Empty-mount mode moves each staging child into the mount with
    /// no-replace; on a mid-loop failure it attempts to move published names
    /// back into staging before returning Err.
    pub(crate) fn publish(self) -> io::Result<()> {
        match &self.kind {
            PublishKind::RenameDirectory { name } => {
                let result = rustix::fs::renameat_with(
                    &self.parent,
                    self.temporary
                        .path()
                        .file_name()
                        .expect("generated staging name"),
                    &self.parent,
                    name,
                    rustix::fs::RenameFlags::NOREPLACE,
                );
                match result {
                    Ok(()) => {
                        let _ = self.temporary.keep();
                        Ok(())
                    }
                    Err(error) => Err(error.into()),
                }
            }
            PublishKind::MoveChildren => self.publish_children(),
        }
    }

    fn publish_children(self) -> io::Result<()> {
        let staging_dir = File::open(self.temporary.path())?;
        let mut names: Vec<OsString> = fs::read_dir(self.temporary.path())?
            .map(|entry| entry.map(|e| e.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        names.sort();
        let mut published = Vec::new();
        for name in &names {
            match rustix::fs::renameat_with(
                &staging_dir,
                name.as_os_str(),
                &self.parent,
                name.as_os_str(),
                rustix::fs::RenameFlags::NOREPLACE,
            ) {
                Ok(()) => published.push(name.clone()),
                Err(error) => {
                    for done in published.iter().rev() {
                        let _ = rustix::fs::renameat(
                            &self.parent,
                            done.as_os_str(),
                            &staging_dir,
                            done.as_os_str(),
                        );
                    }
                    return Err(error.into());
                }
            }
        }
        self.temporary.close()?;
        Ok(())
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
        let digest = Digest::Sha256(Sha256::digest(&bytes).into());
        for (input, size) in [
            (&bytes[..bytes.len() - 1], bytes.len() as u64),
            (&bytes[..], bytes.len() as u64 - 1),
            (&bytes[..], bytes.len() as u64 + 1),
        ] {
            assert!(staged.store_verified(input, &digest, size).is_err());
            assert!(!staged.blob_path(&digest).exists());
        }
        let wrong = Digest::Sha256(Sha256::digest(b"wrong").into());
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
        // Replacing an already staged digest must also remove partial bytes
        // when verification fails, so they cannot survive into publication.
        assert!(
            staged
                .store_verified(&bytes[..bytes.len() - 1], &digest, bytes.len() as u64)
                .is_err()
        );
        assert!(!staged.blob_path(&digest).exists());
        staged.discard().unwrap();
    }
    #[test]
    fn late_collision_preserves_existing_output_and_cleans_staging() {
        let parent = tempfile::tempdir().unwrap();
        let destination = parent.path().join("output");
        let staged = StagedDirectory::create(&destination).unwrap();
        staged.store_blob(b"payload").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("sentinel"), b"existing").unwrap();
        let error = staged.publish().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(fs::read(destination.join("sentinel")).unwrap(), b"existing");
        assert_eq!(fs::read_dir(parent.path()).unwrap().count(), 1);
    }
    #[test]
    fn cache_mutation_does_not_install_invalid_blob() {
        let parent = tempfile::tempdir().unwrap();
        let cache_path = parent.path().join("cache");
        fs::create_dir_all(cache_path.join("blobs/sha256")).unwrap();
        let digest = Digest::Sha256(Sha256::digest(b"original").into());
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
        staged
            .write_file(&staged.temporary.path().join("index.json"), b"index")
            .unwrap();
        let moved = root.path().join("moved");
        fs::rename(&parent, &moved).unwrap();
        fs::create_dir(&parent).unwrap();
        staged.publish().unwrap();
        assert!(!parent.join("output").exists());
        assert_eq!(fs::read(moved.join("output/index.json")).unwrap(), b"index");
        assert_eq!(
            fs::read(moved.join("output/blobs/sha256").join(digest.hex())).unwrap(),
            b"{}"
        );
        assert!(StagedDirectory::create(&moved.join("output")).is_err());
    }

    #[test]
    fn empty_existing_directory_publishes_children_into_mount() {
        let parent = tempfile::TempDir::new().unwrap();
        let mount = parent.path().join("output");
        fs::create_dir(&mount).unwrap();
        let staged = StagedDirectory::create(&mount).unwrap();
        staged
            .write_file(&staged.temporary.path().join("index.json"), b"index")
            .unwrap();
        staged.publish().unwrap();
        assert_eq!(fs::read(mount.join("index.json")).unwrap(), b"index");
        assert!(mount.join("blobs/sha256").is_dir());
        let leftover_staging = fs::read_dir(&mount)
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".conglobate-output-")
            });
        assert!(
            !leftover_staging,
            "staging directory must be removed after publish"
        );
    }

    #[test]
    fn non_empty_existing_directory_is_rejected() {
        let parent = tempfile::TempDir::new().unwrap();
        let mount = parent.path().join("output");
        fs::create_dir(&mount).unwrap();
        fs::write(mount.join("sentinel"), b"x").unwrap();
        let error = StagedDirectory::create(&mount).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
    }
}
