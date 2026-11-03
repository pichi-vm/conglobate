// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Cache, CacheError, VerifiedMetadata};
use crate::artifact::{MEDIA_TYPE_PICHI_ARTIFACT_V1, Manifest};
use crate::image::IndexDigest;
use oci_spec::image::{Digest, ImageIndex, MediaType, Platform};

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
        platform.os() == &"pichi".into()
            && platform.architecture() == &self.oci_name().into()
            && match platform.variant().as_deref() {
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
        if config.media_type() != &MediaType::EmptyJSON {
            self.verify(config.digest(), config.size())?;
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
        Ok(())
    }

    pub(crate) fn resolve(&self, pin: IndexDigest) -> Result<ResolvedImage, CacheError> {
        let architecture = HostArchitecture::current()?;
        let index = self.metadata(
            &format!("sha256:{}", hex::encode(*pin.bytes()))
                .parse::<Digest>()
                .expect("SHA-256 digest"),
            None,
        )?;
        let parsed: ImageIndex = serde_json::from_slice(index.bytes())?;
        if parsed.schema_version() != 2
            || parsed
                .media_type()
                .as_ref()
                .is_some_and(|media_type| media_type != &MediaType::ImageIndex)
        {
            return Err(CacheError::Invalid(
                "lock must reference an OCI image index",
            ));
        }
        let mut candidates = parsed.manifests().iter().filter(|descriptor| {
            descriptor.artifact_type().as_ref().map(AsRef::as_ref)
                == Some(MEDIA_TYPE_PICHI_ARTIFACT_V1)
                && descriptor
                    .platform()
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
        if selected.media_type() != &MediaType::ImageManifest {
            return Err(CacheError::Invalid(
                "selected descriptor is not an image manifest",
            ));
        }
        let manifest_bytes = self.metadata(selected.digest(), Some(selected.size()))?;
        let manifest: Manifest = serde_json::from_slice(manifest_bytes.bytes())?;
        manifest.validate()?;
        // The shared format validates artifact contents, not the OCI envelope.
        if manifest.schema_version != 2
            || manifest.media_type != MediaType::ImageManifest.to_string()
        {
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
                use crate::cache::tests::ScuteFixture as _;
                let root = data.scute_root(parent);
                let data_blocks =
                    std::num::NonZero::new((data.len() as u64).div_ceil(4096)).unwrap();
                let bytes = if compressed {
                    b"unread encoded scute".to_vec()
                } else {
                    data
                };
                let digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
                    .parse::<Digest>()
                    .expect("SHA-256 digest");
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
            let pmi = b"opaque PMI".to_vec();
            let config = ruzstd::encoding::compress_to_vec(
                b"kernel.quiet\n".as_slice(),
                ruzstd::encoding::CompressionLevel::Fastest,
            );
            for (media, bytes) in [
                ("pmi.v1", pmi.as_slice()),
                ("bootconfig.v1+zstd", config.as_slice()),
            ] {
                let digest = format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
                    .parse::<Digest>()
                    .expect("SHA-256 digest");
                layers.push(
                    serde_json::from_value(serde_json::json!({
                        "mediaType": format!("application/vnd.pichi.{media}"),
                        "digest": digest.to_string(), "size": bytes.len()
                    }))
                    .unwrap(),
                );
                blobs.push((digest, bytes.to_vec()));
            }
            let config = b"{\"requirements\":{}}".to_vec();
            let config_digest = format!("sha256:{}", hex::encode(Sha256::digest(&config)))
                .parse::<Digest>()
                .expect("SHA-256 digest");
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
                media_type: MediaType::ImageManifest.to_string().into(),
                artifact_type: MEDIA_TYPE_PICHI_ARTIFACT_V1.into(),
                config: oci_spec::image::Descriptor::new(
                    crate::contract::resources::MEDIA_TYPE_PICHI_CONFIG_V1.into(),
                    config.len() as u64,
                    (config_digest.to_string())
                        .parse::<oci_spec::image::Digest>()
                        .expect("config digest"),
                ),
                layers,
                annotations,
            };
            blobs.push((config_digest, config));
            for (digest, bytes) in &blobs {
                fs::write(
                    directory.path().join("blobs/sha256").join(digest.digest()),
                    bytes,
                )
                .unwrap();
            }
            let bytes = serde_json::to_vec(&manifest).unwrap();
            let image = ResolvedImage {
                manifest_bytes: VerifiedMetadata {
                    digest: format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
                        .parse::<Digest>()
                        .expect("SHA-256 digest"),
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
                .join(digest.digest());
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
