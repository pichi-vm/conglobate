// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{BlobReceipt, Cache, CacheError, VerifiedMetadata};
use crate::image::IndexDigest;
use pichi_artifact::{Digest, MEDIA_TYPE_PICHI_ARTIFACT_V1, Manifest};
use serde::Deserialize;

const INDEX: &str = "application/vnd.oci.image.index.v1+json";
const MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";

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
    digest: String,
    size: u64,
    platform: Option<Platform>,
}
#[derive(Debug, Deserialize)]
struct Platform {
    os: String,
    architecture: String,
    variant: Option<String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostArchitecture {
    Amd64,
    Arm64,
}
impl HostArchitecture {
    pub fn current() -> Result<Self, CacheError> {
        match std::env::consts::ARCH {
            "x86_64" => Ok(Self::Amd64),
            "aarch64" => Ok(Self::Arm64),
            _ => Err(CacheError::Invalid("unsupported host architecture")),
        }
    }
    pub fn oci_name(self) -> &'static str {
        match self {
            Self::Amd64 => "amd64",
            Self::Arm64 => "arm64",
        }
    }
    fn matches(self, platform: &Platform) -> bool {
        platform.os == "pichi"
            && platform.architecture == self.oci_name()
            && match platform.variant.as_deref() {
                None | Some("") => true,
                Some("v8") => self == Self::Arm64,
                _ => false,
            }
    }
}

/// Captured, verified metadata only. This does not prove scute chain integrity.
#[derive(Debug)]
pub struct ResolvedImage {
    index: VerifiedMetadata,
    manifest_bytes: VerifiedMetadata,
    manifest: Manifest,
    architecture: HostArchitecture,
}
impl ResolvedImage {
    pub fn index(&self) -> &VerifiedMetadata {
        &self.index
    }
    pub fn manifest_bytes(&self) -> &VerifiedMetadata {
        &self.manifest_bytes
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn architecture(&self) -> HostArchitecture {
        self.architecture
    }
}
impl Cache {
    /// Check every non-inline content descriptor in the selected manifest.
    /// This is content identity verification, not scute-chain verification.
    pub fn verify_references(&self, image: &ResolvedImage) -> Result<Vec<BlobReceipt>, CacheError> {
        let manifest = image.manifest();
        let config = &manifest.config;
        let mut receipts = Vec::new();
        if !config.is_empty() {
            let digest = config
                .digest
                .parse()
                .map_err(|_| CacheError::Invalid("invalid config digest"))?;
            receipts.push(self.verify(&digest, config.size)?);
        }
        for layer in &manifest.layers {
            let digest = layer
                .digest_str()
                .parse()
                .map_err(|_| CacheError::Invalid("invalid layer digest"))?;
            receipts.push(self.verify(&digest, layer.size())?);
        }
        Ok(receipts)
    }
    pub fn resolve(&self, pin: IndexDigest) -> Result<ResolvedImage, CacheError> {
        let architecture = HostArchitecture::current()?;
        let index = self.metadata(&Digest::Sha256(*pin.bytes()), None)?;
        let parsed: Index = serde_json::from_slice(index.bytes())?;
        if parsed.schema_version != 2 || parsed.media_type != INDEX {
            return Err(CacheError::Invalid(
                "lock must reference an OCI image index",
            ));
        }
        let mut candidates = parsed.manifests.iter().filter(|descriptor| {
            descriptor.artifact_type.as_deref() == Some(MEDIA_TYPE_PICHI_ARTIFACT_V1)
                && descriptor
                    .platform
                    .as_ref()
                    .is_some_and(|p| architecture.matches(p))
        });
        let selected = candidates
            .next()
            .ok_or(CacheError::Invalid("index has no compatible host platform"))?;
        if candidates.next().is_some() {
            return Err(CacheError::Invalid(
                "index has ambiguous host platform entries",
            ));
        }
        if selected.media_type != MANIFEST {
            return Err(CacheError::Invalid(
                "selected descriptor is not an image manifest",
            ));
        }
        let digest = selected
            .digest
            .parse()
            .map_err(|_| CacheError::Invalid("invalid manifest digest"))?;
        let manifest_bytes = self.metadata(&digest, Some(selected.size))?;
        let manifest = Manifest::from_reader_validated(manifest_bytes.bytes())
            .map_err(|error| CacheError::Manifest(error.to_string()))?;
        // The shared format validates artifact contents, not the OCI envelope.
        if manifest.schema_version != 2 || manifest.media_type != MANIFEST {
            return Err(CacheError::Invalid("invalid manifest envelope"));
        }
        Ok(ResolvedImage {
            index,
            manifest_bytes,
            manifest,
            architecture,
        })
    }
}
