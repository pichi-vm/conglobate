// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Read-only, digest-verified access to the pichi content store.

use crate::{Error, recipe::IndexDigest};
use pichi_artifact::Manifest;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

const INDEX: &str = "application/vnd.oci.image.index.v1+json";
const MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
/// Ecosystem selector for VM artifacts, independent of the build host's OS.
pub(crate) const PLATFORM_OS: &str = "pichi";
const MAX_METADATA: u64 = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    schema_version: u32,
    media_type: String,
    manifests: Vec<Descriptor>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Descriptor {
    media_type: String,
    artifact_type: Option<String>,
    digest: IndexDigest,
    size: u64,
    platform: Option<Platform>,
}

#[derive(Debug, Deserialize)]
struct Platform {
    os: String,
    architecture: String,
    variant: Option<String>,
}

/// An image description verified against a locked index. Blob contents are
/// verified again when copied to private storage; cache paths are not trusted.
#[derive(Debug)]
pub struct ResolvedImage {
    pub index: String,
    pub manifest_digest: String,
    pub manifest: Manifest,
    pub manifest_bytes: Vec<u8>,
}

/// This type has no cache mutation API.
#[derive(Debug)]
pub struct Cache {
    root: PathBuf,
}

impl Cache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path(&self, digest: &str) -> Result<PathBuf, Error> {
        IndexDigest::try_from(digest.to_owned())?;
        Ok(self.root.join("blobs/sha256").join(&digest[7..]))
    }

    /// Copy and verify in a single pass. The destination must be private scratch:
    /// a failed verification may have written untrusted bytes there.
    pub fn copy_blob(
        &self,
        digest: &str,
        size: u64,
        destination: &mut impl Write,
    ) -> Result<(), Error> {
        // A FIFO must not block before we can reject its file type. Inspect the
        // same descriptor we will read, not a racy pre-open metadata check.
        let file = File::from(
            rustix::fs::open(
                self.path(digest)?,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::NONBLOCK
                    | rustix::fs::OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )
            .map_err(std::io::Error::from)?,
        );
        if !file.metadata()?.is_file() {
            return Err(Error::Invalid(format!(
                "cache blob {digest} is not a regular file"
            )));
        }
        let mut source = file.take(size.saturating_add(1));
        let mut hash = Sha256::new();
        let mut count = 0_u64;
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > size {
                return Err(Error::Invalid(format!("size mismatch for {digest}")));
            }
            hash.update(&buffer[..n]);
            destination.write_all(&buffer[..n])?;
        }
        if count != size || format!("sha256:{:x}", hash.finalize()) != digest {
            return Err(Error::Invalid(format!("content mismatch for {digest}")));
        }
        Ok(())
    }

    fn metadata(&self, digest: &str, size: Option<u64>) -> Result<Vec<u8>, Error> {
        let size = match size {
            Some(size) => size,
            None => std::fs::metadata(self.path(digest)?)?.len(),
        };
        if size > MAX_METADATA {
            return Err(Error::Invalid("OCI metadata exceeds 16 MiB".into()));
        }
        let mut bytes = Vec::new();
        self.copy_blob(digest, size, &mut bytes)?;
        Ok(bytes)
    }

    pub fn resolve(&self, locked: &IndexDigest) -> Result<ResolvedImage, Error> {
        let bytes = self.metadata(locked.as_ref(), None)?;
        let index: Index = serde_json::from_slice(&bytes)?;
        if index.schema_version != 2 || index.media_type != INDEX {
            return Err(Error::Invalid(
                "image lock must reference an OCI image index".into(),
            ));
        }
        let architecture = host_architecture()?;
        let mut candidates = index.manifests.iter().filter(|entry| {
            entry.artifact_type.as_deref() == Some(pichi_artifact::MEDIA_TYPE_PICHI_ARTIFACT_V1)
                && entry.platform.as_ref().is_some_and(|platform| {
                    platform.os == PLATFORM_OS
                        && platform.architecture == architecture
                        && match platform.variant.as_deref() {
                            None | Some("") => true,
                            Some("v8") => architecture == "arm64",
                            _ => false,
                        }
                })
        });
        let selected = candidates
            .next()
            .ok_or_else(|| Error::Invalid("index has no compatible host platform".into()))?;
        if candidates.next().is_some() {
            return Err(Error::Invalid(
                "index has ambiguous host platform entries".into(),
            ));
        }
        if selected.media_type != MANIFEST {
            return Err(Error::Invalid(
                "index entry must reference an image manifest".into(),
            ));
        }
        let manifest_bytes = self.metadata(selected.digest.as_ref(), Some(selected.size))?;
        let manifest = Manifest::from_reader_validated(manifest_bytes.as_slice())
            .map_err(|error| Error::Invalid(format!("image manifest: {error}")))?;
        Ok(ResolvedImage {
            index: locked.as_ref().into(),
            manifest_digest: selected.digest.as_ref().into(),
            manifest,
            manifest_bytes,
        })
    }

    /// Validate all selected manifest blobs without modifying the cache.
    pub fn verify_blobs(&self, manifest: &Manifest) -> Result<(), Error> {
        let config = &manifest.config;
        if !config.is_empty() {
            self.copy_blob(&config.digest, config.size, &mut std::io::sink())?;
        }
        for layer in &manifest.layers {
            self.copy_blob(layer.digest_str(), layer.size(), &mut std::io::sink())?;
        }
        Ok(())
    }
}

pub(crate) fn host_architecture() -> Result<&'static str, Error> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("amd64"),
        "aarch64" => Ok("arm64"),
        other => Err(Error::Invalid(format!(
            "unsupported host architecture {other}"
        ))),
    }
}
