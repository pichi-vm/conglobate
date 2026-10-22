// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Self-contained OCI output finalized privately before atomic publication.

use crate::{
    Error,
    cache::{Cache, ResolvedImage},
    inventory::{Digest, Inventory},
};
use pichi_artifact::Manifest;
use sha2::{Digest as _, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Artifact {
    directory: PathBuf,
}

impl Artifact {
    /// Directory must be under the build's private tmpfs workspace.
    pub fn new(directory: &Path) -> Result<Self, Error> {
        std::fs::create_dir(directory)?;
        std::fs::create_dir_all(directory.join("image.oci/blobs/sha256"))?;
        Ok(Self {
            directory: directory.to_owned(),
        })
    }

    pub fn blob_path(&self, digest: &str) -> Result<PathBuf, Error> {
        crate::recipe::IndexDigest::try_from(digest.to_owned())?;
        Ok(self
            .directory
            .join("image.oci/blobs/sha256")
            .join(&digest[7..]))
    }

    pub fn put(&self, mut reader: impl Read) -> Result<(Digest, u64), Error> {
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)?;
        let mut hash = Sha256::new();
        let mut size = 0;
        let mut buffer = vec![0; 65536];
        loop {
            crate::cancel::check()?;
            let n = reader.read(&mut buffer)?;
            crate::cancel::check()?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
            file.write_all(&buffer[..n])?;
            size += n as u64;
        }
        let digest = Digest {
            algorithm: "sha256".into(),
            digest: hash.finalize().to_vec(),
        };
        let destination = self.blob_path(&digest.oci())?;
        crate::cancel::check()?;
        if !destination.exists() {
            file.persist_noclobber(destination)
                .map_err(|error| error.error)?;
        }
        Ok((digest, size))
    }

    pub fn inherit(&self, cache: &Cache, image: &ResolvedImage) -> Result<(), Error> {
        let config = &image.manifest.config;
        if config.is_empty() {
            self.put(&b"{}"[..])?;
        } else {
            self.copy_verified(cache, &config.digest, config.size)?;
        }
        for layer in &image.manifest.layers {
            self.copy_verified(cache, layer.digest_str(), layer.size())?;
        }
        Ok(())
    }

    fn copy_verified(&self, cache: &Cache, digest: &str, size: u64) -> Result<(), Error> {
        let destination = self.blob_path(digest)?;
        if destination.exists() {
            return Ok(());
        }
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)?;
        cache.copy_blob(digest, size, &mut file)?;
        file.persist_noclobber(destination)
            .map_err(|error| error.error)?;
        Ok(())
    }

    pub fn finalize(&self, manifest: &Manifest, inventory: &Inventory) -> Result<(), Error> {
        manifest
            .validate()
            .map_err(|e| Error::Invalid(format!("output manifest: {e}")))?;
        if manifest.config.is_empty() {
            self.put(&b"{}"[..])?;
        }
        let actual_root = manifest.carapace_verity_hash().map(|root| root.to_vec());
        if actual_root.as_deref()
            != inventory
                .outputs
                .carapace_root
                .as_ref()
                .map(std::convert::AsRef::as_ref)
        {
            return Err(Error::Invalid(
                "inventory carapace root differs from manifest".into(),
            ));
        }
        for (is_pmi, expected) in [
            (true, &inventory.outputs.pmi),
            (false, &inventory.outputs.dtb),
        ] {
            let layer = manifest.layers.iter().find(|layer| {
                if is_pmi {
                    matches!(layer, pichi_artifact::Layer::Pmi(_))
                } else {
                    matches!(layer, pichi_artifact::Layer::Dtb(_))
                }
            });
            if layer.map(pichi_artifact::Layer::digest_str)
                != expected.as_ref().map(Digest::oci).as_deref()
            {
                return Err(Error::Invalid(
                    "inventory firmware differs from manifest".into(),
                ));
            }
        }
        let mut scutes: Vec<_> = manifest
            .layers
            .iter()
            .filter(|layer| {
                matches!(
                    layer,
                    pichi_artifact::Layer::Scute(_) | pichi_artifact::Layer::ScuteZstd(_)
                )
            })
            .map(|layer| layer.digest_str().to_owned())
            .collect();
        let mut recorded: Vec<_> = inventory
            .outputs
            .scutes
            .iter()
            .map(|scute| scute.digest.oci())
            .collect();
        scutes.sort();
        recorded.sort();
        if scutes != recorded {
            return Err(Error::Invalid(
                "inventory scutes differ from manifest".into(),
            ));
        }
        let cache = Cache::new(self.directory.join("image.oci"));
        let bytes = manifest
            .to_bytes()
            .map_err(|e| Error::Invalid(e.to_string()))?;
        cache.verify_blobs(manifest)?;
        if manifest.config.is_empty() {
            cache.copy_blob(
                &manifest.config.digest,
                manifest.config.size,
                &mut std::io::sink(),
            )?;
        }
        let (digest, size) = self.put(bytes.as_slice())?;
        let mut referenced: std::collections::BTreeSet<_> = manifest
            .layers
            .iter()
            .map(|l| l.digest_str().to_owned())
            .collect();
        referenced.insert(manifest.config.digest.clone());
        referenced.insert(digest.oci());
        for entry in std::fs::read_dir(self.directory.join("image.oci/blobs/sha256"))? {
            let entry = entry?;
            if !referenced.contains(&format!("sha256:{}", entry.file_name().to_string_lossy())) {
                std::fs::remove_file(entry.path())?;
            }
        }
        if digest != inventory.outputs.manifest {
            return Err(Error::Invalid(
                "inventory does not bind the output manifest".into(),
            ));
        }
        let architecture = crate::cache::host_architecture()?;
        let index = serde_json::json!({"schemaVersion": 2, "mediaType": "application/vnd.oci.image.index.v1+json",
            "manifests": [{"mediaType": "application/vnd.oci.image.manifest.v1+json", "artifactType": manifest.artifact_type, "digest": digest.oci(), "size": size,
                "platform": {"os": crate::cache::PLATFORM_OS, "architecture": architecture}}]});
        std::fs::write(
            self.directory.join("image.oci/index.json"),
            serde_json::to_vec(&index)?,
        )?;
        std::fs::write(
            self.directory.join("image.oci/oci-layout"),
            b"{\"imageLayoutVersion\":\"1.0.0\"}",
        )?;
        std::fs::write(self.directory.join("inventory.cbor"), inventory.encode()?)?;
        Ok(())
    }

    /// Publish only a finalized artifact. NOREPLACE closes the race between
    /// destination validation and rename, including an existing empty directory.
    pub fn publish(self, destination: &Path) -> Result<(), Error> {
        if !self.directory.join("inventory.cbor").is_file() {
            return Err(Error::Invalid("artifact has not been finalized".into()));
        }
        if std::fs::symlink_metadata(destination).is_ok() {
            return Err(Error::Invalid("output destination already exists".into()));
        }
        let parent = destination
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let staging = tempfile::Builder::new()
            .prefix(".conglobate-output-")
            .permissions(std::os::unix::fs::PermissionsExt::from_mode(0o700))
            .tempdir_in(parent)?;
        copy_tree(&self.directory, staging.path())?;
        crate::cancel::check()?;
        rustix::fs::renameat_with(
            rustix::fs::CWD,
            staging.path(),
            rustix::fs::CWD,
            destination,
            rustix::fs::RenameFlags::NOREPLACE,
        )
        .map_err(std::io::Error::from)?;
        Ok(())
    }
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), Error> {
    for entry in std::fs::read_dir(source)? {
        crate::cancel::check()?;
        let entry = entry?;
        let target = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir(&target)?;
            copy_tree(&entry.path(), &target)?;
        } else if entry.file_type()?.is_file() {
            let mut input = File::open(entry.path())?;
            let mut output = File::create(target)?;
            let mut buffer = vec![0; 65536];
            loop {
                crate::cancel::check()?;
                let count = input.read(&mut buffer)?;
                crate::cancel::check()?;
                if count == 0 {
                    break;
                }
                output.write_all(&buffer[..count])?;
            }
            output.sync_all()?;
        } else {
            return Err(Error::Invalid(
                "unexpected file type in private artifact".into(),
            ));
        }
    }
    File::open(destination)?.sync_all()?;
    Ok(())
}
