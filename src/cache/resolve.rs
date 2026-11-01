// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Cache, CacheError, VerifiedMetadata};
use crate::artifact::{Digest, MEDIA_TYPE_PICHI_ARTIFACT_V1, Manifest};
use crate::artifact::{
    Index, MEDIA_TYPE_OCI_INDEX_V1 as INDEX, MEDIA_TYPE_OCI_MANIFEST_V1 as MANIFEST, Platform,
};
use crate::image::IndexDigest;

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
    root: Option<[u8; 32]>,
}
impl ResolvedImage {
    /// The commitment in the authenticated manifest, not a payload receipt.
    pub(crate) fn root(&self) -> Option<[u8; 32]> {
        self.root
    }

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
    /// Authenticate small non-scute inputs. Scutes are supplied separately by
    /// Pichi and checked lazily by Carapace against the manifest commitment.
    pub(crate) fn verify_metadata(&self, image: &ResolvedImage) -> Result<(), CacheError> {
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
            if matches!(
                layer,
                crate::artifact::Layer::Scute(_) | crate::artifact::Layer::ScuteZstd(_)
            ) {
                continue;
            }
            let digest = layer
                .digest_str()
                .parse()
                .map_err(|_| CacheError::Invalid("invalid layer digest"))?;
            self.verify(&digest, layer.size())?;
        }
        if let Some(config) = manifest.layers.iter().find_map(|layer| match layer {
            crate::artifact::Layer::BootConfig(config) => Some(config),
            _ => None,
        }) {
            let digest = config
                .digest
                .parse()
                .map_err(|_| CacheError::Invalid("invalid bootconfig digest"))?;
            let data = self.metadata(&digest, Some(config.size))?;
            let text = std::str::from_utf8(data.bytes())
                .map_err(|_| CacheError::Invalid("bootconfig is not UTF-8"))?;
            let parsed: crate::artifact::BootConfig = text.parse()?;
            if parsed.is_empty() {
                return Err(CacheError::Invalid("empty bootconfig must be omitted"));
            }
            let pmi = manifest
                .layers
                .iter()
                .find_map(|layer| match layer {
                    crate::artifact::Layer::Pmi(pmi) => Some(pmi),
                    _ => None,
                })
                .ok_or(CacheError::Invalid("bootconfig requires PMI"))?;
            let digest = pmi
                .digest
                .parse()
                .map_err(|_| CacheError::Invalid("invalid PMI digest"))?;
            let mut bytes = Vec::new();
            let size = usize::try_from(pmi.size)
                .map_err(|_| CacheError::Invalid("PMI exceeds address space"))?;
            bytes
                .try_reserve_exact(size)
                .map_err(|_| CacheError::Invalid("PMI allocation failed"))?;
            self.copy_verified(&digest, pmi.size, &mut bytes)?;
            crate::firmware::BootConfigContract::from_pmi(&bytes, image.architecture().oci_name())?
                .check_size(text.len())?;
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
        let manifest_bytes = self.metadata(&digest, Some(selected.size))?;
        let manifest: Manifest = serde_json::from_slice(manifest_bytes.bytes())?;
        manifest.validate()?;
        // The shared format validates artifact contents, not the OCI envelope.
        if manifest.schema_version != 2 || manifest.media_type != MANIFEST {
            return Err(CacheError::Invalid("invalid manifest envelope"));
        }
        Ok(ResolvedImage {
            manifest_bytes,
            root: manifest.carapace_root()?,
            manifest,
            architecture,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{Layer, ScuteAnnotations, ScuteDescriptor};
    use sha2::{Digest as _, Sha256};
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
                use crate::tests::ScuteFixture as _;
                let root = data.scute_root(parent);
                let data_blocks =
                    std::num::NonZero::new((data.len() as u64).div_ceil(4096)).unwrap();
                let bytes = if compressed {
                    b"unread encoded scute".to_vec()
                } else {
                    data
                };
                let digest = Digest::Sha256(Sha256::digest(&bytes).into());
                let descriptor = ScuteDescriptor {
                    digest: digest.to_string(),
                    size: bytes.len() as u64,
                    annotations: ScuteAnnotations {
                        data_blocks,
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
            // Identity verification is distinct from firmware format validation.
            for (media, bytes) in [("pmi", b"firmware".as_slice()), ("dtb", b"tree".as_slice())] {
                let digest = Digest::Sha256(Sha256::digest(bytes).into());
                layers.push(
                    serde_json::from_value(serde_json::json!({
                        "mediaType": format!("application/vnd.pichi.{media}.v1"),
                        "digest": digest.to_string(), "size": bytes.len()
                    }))
                    .unwrap(),
                );
                blobs.push((digest, bytes.to_vec()));
            }
            let config = b"{\"requirements\":{}}".to_vec();
            let config_digest = Digest::Sha256(Sha256::digest(&config).into());
            let annotations: BTreeMap<String, String> = [
                ("algo", "sha256"),
                ("data-block-size", "4096"),
                ("hash-block-size", "4096"),
                ("version", "1"),
                ("hash-type", "1"),
                ("hash", &hex::encode(parent)),
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
            let bytes = serde_json::to_vec(&manifest).unwrap();
            let image = ResolvedImage {
                manifest_bytes: VerifiedMetadata {
                    digest: Digest::Sha256(Sha256::digest(&bytes).into()),
                    bytes,
                },
                root: manifest.carapace_root().unwrap(),
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
    fn metadata_checks_do_not_scan_scute_payloads() {
        let fixture = ContentFixture::new();
        let cache = Cache::open(fixture.directory.path()).unwrap();
        cache.verify_metadata(&fixture.image).unwrap();
        for (digest, bytes) in &fixture.blobs {
            let path = fixture
                .directory
                .path()
                .join("blobs/sha256")
                .join(digest.hex());
            let mut changed = bytes.clone();
            changed[0] ^= 1;
            fs::write(&path, changed).unwrap();
            let scute = fixture.image.manifest().layers.iter().any(|layer| {
                matches!(layer, Layer::Scute(_) | Layer::ScuteZstd(_))
                    && layer.digest_str() == digest.to_string()
            });
            assert_eq!(
                cache.verify_metadata(&fixture.image).is_ok(),
                scute,
                "{digest}"
            );
            // Export still authenticates encoded bytes as it copies them.
            assert!(
                cache
                    .copy_verified(digest, bytes.len() as u64, &mut std::io::sink())
                    .is_err()
            );
            fs::write(path, bytes).unwrap();
        }
    }
}
