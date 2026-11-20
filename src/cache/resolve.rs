// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Cache, CacheError, VerifiedMetadata};
use crate::artifact::Media as _;

use crate::artifact::{Digest, Manifest, PichiArtifact};
use crate::artifact::{Index, Platform};
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
}
impl Cache {
    /// Snapshot local tag mappings once; they name inputs but do not authenticate them.
    pub(crate) fn tags(&self) -> Result<Index, CacheError> {
        let bytes =
            Self::read_metadata_file(self.regular_file(std::path::Path::new("index.json"))?, None)?;
        let index: Index = serde_json::from_slice(&bytes)?;
        Ok(index)
    }

    /// Authenticate metadata and require regular scute files of the declared size.
    /// Scute payloads are checked lazily by Carapace, without scanning them here.
    pub(crate) fn verify_metadata(&self, image: &ResolvedImage) -> Result<(), CacheError> {
        let manifest = image.manifest();
        let config = &manifest.config;
        self.verify(&config.digest, config.size)?;
        for layer in &manifest.layers {
            let digest = layer.digest();
            if matches!(layer, crate::artifact::Layer::Scute(_)) {
                if self.blob(digest)?.metadata()?.len() != layer.size() {
                    return Err(CacheError::Invalid("scute size mismatch"));
                }
            } else {
                self.verify(digest, layer.size())?;
            }
        }
        Ok(())
    }

    pub(crate) fn resolve(&self, pin: IndexDigest) -> Result<ResolvedImage, CacheError> {
        let architecture = HostArchitecture::current()?;
        let index = self.metadata(&Digest::Sha256(*pin.bytes()), None)?;
        let parsed: Index = serde_json::from_slice(index.bytes())?;
        let mut candidates = parsed.manifests.iter().filter(|descriptor| {
            descriptor.artifact_type.as_deref() == Some(PichiArtifact::MEDIA_TYPE)
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
        if selected.media_type != Manifest::MEDIA_TYPE {
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
        Ok(ResolvedImage {
            manifest_bytes,
            root: manifest.carapace_root()?,
            manifest,
        })
    }
}

impl crate::image::ImageReference {
    pub(crate) fn cached_index(&self, tags: &Index) -> Result<IndexDigest, CacheError> {
        if let Some((_, digest)) = self.as_ref().split_once('@') {
            return digest
                .parse()
                .map_err(|_| CacheError::Invalid("invalid bootstrap digest"));
        }
        let mut matches = tags.manifests.iter().filter(|entry| {
            entry
                .annotations
                .get("org.opencontainers.image.ref.name")
                .is_some_and(|name| name == self.as_ref())
        });
        let selected = matches
            .next()
            .ok_or(CacheError::Invalid("bootstrap image is not in local cache"))?;
        if matches.next().is_some() {
            return Err(CacheError::Invalid(
                "ambiguous bootstrap tag in local cache",
            ));
        }
        selected
            .digest
            .parse()
            .map_err(|_| CacheError::Invalid("invalid cached bootstrap digest"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::{Layer, ScuteAnnotations, ScuteDescriptor};
    use sha2::{Digest as _, Sha256};
    use std::fs;

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
                use crate::cache::tests::ScuteFixture as _;
                let (_, tree, root) = data.scute_files(parent);
                let data_digest = Digest::Sha256(Sha256::digest(&data).into());
                let data_size = data.len();
                let data_blocks =
                    std::num::NonZero::new((data.len() as u64).div_ceil(4096)).unwrap();
                let bytes = if compressed {
                    b"unread encoded scute".to_vec()
                } else {
                    data
                };
                let digest = Digest::Sha256(Sha256::digest(&bytes).into());
                let descriptor = ScuteDescriptor {
                    media_type: crate::artifact::Compressable::Raw(Default::default()),
                    digest: digest.clone(),
                    size: bytes.len() as u64,
                    annotations: ScuteAnnotations {
                        digest: data_digest,
                        size: std::num::NonZeroU64::new(data_size as u64).unwrap().into(),
                        verity_digest: Digest::Sha256(Sha256::digest(&tree).into()),
                        data_blocks: data_blocks.into(),
                        salt: devmap_verity::Salt::new(&parent).unwrap().into(),
                    },
                };
                layers.push(if compressed {
                    Layer::Scute(crate::artifact::ScuteDescriptor {
                        media_type: crate::artifact::Compressable::Zstd(Default::default()),
                        ..descriptor
                    })
                } else {
                    Layer::Scute(crate::artifact::ScuteDescriptor {
                        media_type: crate::artifact::Compressable::Raw(Default::default()),
                        ..descriptor
                    })
                });
                blobs.push((digest, bytes));
                parent = root;
            }
            let pmi = b"opaque PMI".to_vec();
            let config = ruzstd::encoding::compress_to_vec(
                b"kernel.quiet\n".as_slice(),
                ruzstd::encoding::CompressionLevel::Fastest,
            );
            for (media, bytes) in [
                ("pmi.v1", pmi.as_slice()),
                ("bootconfig.v1+zstd", config.as_slice()),
            ] {
                let digest = Digest::Sha256(Sha256::digest(bytes).into());
                layers.push(
                    serde_json::from_value(serde_json::json!({
                        "mediaType": format!("application/vnd.pichi.{media}"),
                        "digest": digest.to_string(), "size": bytes.len()
                    }))
                    .unwrap(),
                );
                blobs.push((digest, bytes.to_vec()));
            }
            let config = serde_json::to_vec(&crate::artifact::Config::default()).unwrap();
            let config_digest = Digest::Sha256(Sha256::digest(&config).into());
            let annotations = crate::artifact::ManifestAnnotations {
                oci: Default::default(),
                build: crate::cache::tests::provenance(),
                verity: crate::artifact::CarapaceVerity {
                    root: Some(Digest::Sha256(parent)),
                    ..Default::default()
                },
            };
            let manifest = Manifest {
                schema_version: Default::default(),
                media_type: Default::default(),
                artifact_type: Default::default(),
                config: crate::artifact::ConfigDescriptor::for_config(
                    config_digest.clone(),
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
                manifest_bytes: VerifiedMetadata { bytes },
                root: manifest.carapace_root().unwrap(),
                manifest,
            };
            Self {
                directory,
                image,
                blobs,
            }
        }
    }

    #[test]
    fn metadata_checks_require_supplied_scute_extents() {
        for index in 0..2 {
            for damage in ["missing", "short", "long", "directory"] {
                let fixture = ContentFixture::new();
                let (digest, original) = &fixture.blobs[index];
                let path = fixture
                    .directory
                    .path()
                    .join("blobs/sha256")
                    .join(digest.hex());
                fs::remove_file(&path).unwrap();
                match damage {
                    "missing" => {}
                    "short" => fs::write(&path, &original[..original.len() - 1]).unwrap(),
                    "long" => {
                        let mut bytes = original.clone();
                        bytes.push(0);
                        fs::write(&path, bytes).unwrap();
                    }
                    "directory" => fs::create_dir(&path).unwrap(),
                    _ => unreachable!(),
                }
                let cache = Cache::open(fixture.directory.path()).unwrap();
                assert!(
                    cache.verify_metadata(&fixture.image).is_err(),
                    "{index}: {damage}"
                );
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
            let scute = fixture
                .image
                .manifest()
                .layers
                .iter()
                .any(|layer| matches!(layer, Layer::Scute(_)) && layer.digest() == digest);
            assert_eq!(
                cache.verify_metadata(&fixture.image).is_ok(),
                scute,
                "{digest}"
            );
            // Explicit byte reads still authenticate the encoded content.
            assert!(
                cache
                    .copy_verified(digest, bytes.len() as u64, &mut std::io::sink())
                    .is_err()
            );
            fs::write(path, bytes).unwrap();
        }
    }
}
