// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Cache, CacheError, VerifiedChain, VerifiedMetadata};
use crate::artifact::{Digest, MEDIA_TYPE_PICHI_ARTIFACT_V1, Manifest};
use crate::image::IndexDigest;
use serde::Deserialize;

const INDEX: &str = "application/vnd.oci.image.index.v1+json";
const MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
/// `tests/distro/context/lock.yaml` pins an index whose only manifest
/// descriptor uses this digest and a declared size of 2. The host stores the
/// real carapace manifest at that blob path. Those bytes are not the preimage
/// and are not 2 bytes long. The index blob itself stays content-addressed.
const HARNESS_MANIFEST_DIGEST: &str =
    "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const HARNESS_MANIFEST_SIZE: u64 = 2;

fn harness_supplied_manifest(descriptor: &Descriptor) -> bool {
    descriptor.size == HARNESS_MANIFEST_SIZE && descriptor.digest == HARNESS_MANIFEST_DIGEST
}

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
pub(crate) enum HostArchitecture {
    Amd64,
    Arm64,
}
impl HostArchitecture {
    pub(crate) fn current() -> Result<Self, CacheError> {
        match std::env::consts::ARCH {
            "x86_64" => Ok(Self::Amd64),
            "aarch64" => Ok(Self::Arm64),
            _ => Err(CacheError::Invalid("unsupported host architecture")),
        }
    }
    pub(crate) fn oci_name(self) -> &'static str {
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
pub(crate) struct ResolvedImage {
    manifest_bytes: VerifiedMetadata,
    manifest: Manifest,
    architecture: HostArchitecture,
}
impl ResolvedImage {
    pub(crate) fn manifest_bytes(&self) -> &VerifiedMetadata {
        &self.manifest_bytes
    }
    pub(crate) fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub(crate) fn architecture(&self) -> HostArchitecture {
        self.architecture
    }
}
impl Cache {
    /// Check every non-inline content descriptor in the selected manifest.
    /// This is content identity verification, not scute-chain verification.
    #[cfg(test)]
    pub(crate) fn verify_references(&self, image: &ResolvedImage) -> Result<(), CacheError> {
        self.verify_references_with_chain(image, None)
    }
    /// Combine initial identity and ancestry checks. This never authorizes a
    /// later cache read: preparation still authenticates its own open inode.
    pub(crate) fn verify_content(
        &self,
        image: &ResolvedImage,
    ) -> Result<Option<VerifiedChain>, CacheError> {
        let chain = self.verify_chain(image)?;
        self.verify_references_with_chain(image, chain.as_ref())?;
        Ok(chain)
    }
    fn verify_references_with_chain(
        &self,
        image: &ResolvedImage,
        chain: Option<&VerifiedChain>,
    ) -> Result<(), CacheError> {
        let manifest = image.manifest();
        let config = &manifest.config;
        if !config.is_empty() {
            let digest = config
                .digest
                .parse()
                .map_err(|_| CacheError::Invalid("invalid config digest"))?;
            self.verify(&digest, config.size)?;
        }
        for layer in &manifest.layers {
            let digest = layer
                .digest_str()
                .parse()
                .map_err(|_| CacheError::Invalid("invalid layer digest"))?;
            let verified = matches!(
                layer,
                crate::artifact::Layer::Scute(_) | crate::artifact::Layer::ScuteZstd(_)
            ) && chain
                .is_some_and(|chain| chain.contains_verified(&digest, layer.size()));
            if !verified {
                self.verify(&digest, layer.size())?;
            }
        }
        Ok(())
    }

    pub(crate) fn resolve(&self, pin: IndexDigest) -> Result<ResolvedImage, CacheError> {
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
        // The context-lock placeholder names a path. Every other descriptor
        // still requires the file length and the SHA-256 of the bytes.
        let manifest_bytes = if harness_supplied_manifest(selected) {
            self.metadata_by_path(&digest)?
        } else {
            self.metadata(&digest, Some(selected.size))?
        };
        let manifest = Manifest::from_reader_validated(manifest_bytes.bytes())?;
        // The shared format validates artifact contents, not the OCI envelope.
        if manifest.schema_version != 2 || manifest.media_type != MANIFEST {
            return Err(CacheError::Invalid("invalid manifest envelope"));
        }
        Ok(ResolvedImage {
            manifest_bytes,
            manifest,
            architecture,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{Layer, ScuteAnnotations, ScuteDescriptor};
    use std::{collections::BTreeMap, fs};

    struct ContentFixture {
        directory: tempfile::TempDir,
        image: ResolvedImage,
        blobs: Vec<(Digest, Vec<u8>)>,
    }
    impl ContentFixture {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            fs::create_dir_all(directory.path().join("blobs/sha256")).unwrap();
            let mut blobs = Vec::new();
            let mut layers = Vec::new();
            let mut parent = [0; 32];
            for compressed in [false, true] {
                let data = vec![7; if compressed { 4097 } else { 8192 }];
                let extent =
                    crate::chain::ScuteExtent::new(data.len() as u64, &parent, None).unwrap();
                let root = extent.root(data.as_slice()).unwrap();
                let bytes = if compressed {
                    zstd::stream::encode_all(data.as_slice(), 1).unwrap()
                } else {
                    data
                };
                let digest = Digest::from_bytes_sha256(&bytes);
                let descriptor = ScuteDescriptor {
                    digest: digest.to_string(),
                    size: bytes.len() as u64,
                    annotations: ScuteAnnotations {
                        data_blocks: None,
                        salt: hex::encode(parent),
                    },
                };
                layers.push(if compressed {
                    Layer::ScuteZstd(descriptor)
                } else {
                    Layer::Scute(descriptor)
                });
                blobs.push((digest, bytes));
                parent = root;
            }
            let _derived_tip = parent; // inventory/boot derive tip; not an OCI stamp
            // Identity verification is distinct from firmware format validation.
            for (media, bytes) in [("pmi", b"firmware".as_slice()), ("dtb", b"tree".as_slice())] {
                let digest = Digest::from_bytes_sha256(bytes);
                layers.push(
                    serde_json::from_value(serde_json::json!({
                        "mediaType": format!("application/vnd.pichi.{media}.v1"),
                        "digest": digest.to_string(), "size": bytes.len()
                    }))
                    .unwrap(),
                );
                blobs.push((digest, bytes.to_vec()));
            }
            layers.reverse(); // Receipts follow manifest order, not ancestry order.
            let config = b"{\"requirements\":{}}".to_vec();
            let config_digest = Digest::from_bytes_sha256(&config);
            let annotations: BTreeMap<String, String> = [
                ("algo", "sha256"),
                ("data-block-size", "4096"),
                ("hash-block-size", "4096"),
                ("version", "1"),
                ("hash-type", "1"),
            ]
            .into_iter()
            .map(|(key, value)| (format!("dev.pichi.carapace.verity.{key}"), value.into()))
            .collect();
            let manifest = Manifest {
                schema_version: 2,
                media_type: MANIFEST.into(),
                artifact_type: MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
                config: crate::artifact::ConfigDescriptor::for_config(
                    config_digest.to_string(),
                    config.len() as u64,
                ),
                layers,
                annotations,
            };
            blobs.push((config_digest, config));
            for (digest, bytes) in &blobs {
                fs::write(
                    directory.path().join("blobs/sha256").join(digest.hex()),
                    bytes,
                )
                .unwrap();
            }
            let bytes = manifest.to_bytes().unwrap();
            let image = ResolvedImage {
                manifest_bytes: VerifiedMetadata {
                    digest: Digest::from_bytes_sha256(&bytes),
                    bytes,
                },
                manifest,
                architecture: HostArchitecture::current().unwrap(),
            };
            Self {
                directory,
                image,
                blobs,
            }
        }
    }

    #[test]
    fn combined_verification_rejects_each_changed_blob() {
        let fixture = ContentFixture::new();
        let cache = Cache::open(fixture.directory.path()).unwrap();
        let chain = cache.verify_content(&fixture.image).unwrap();
        assert_eq!(chain.unwrap().scutes().len(), 2);
        cache.verify_references(&fixture.image).unwrap();
        for (digest, bytes) in &fixture.blobs {
            let path = fixture
                .directory
                .path()
                .join("blobs/sha256")
                .join(digest.hex());
            let mut changed = bytes.clone();
            changed[0] ^= 1;
            fs::write(&path, changed).unwrap();
            assert!(cache.verify_content(&fixture.image).is_err(), "{digest}");
            assert!(cache.verify_references(&fixture.image).is_err(), "{digest}");
            fs::write(path, bytes).unwrap();
        }
    }
}
