// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Private scute staging, chain verification, and ecosystem format adapters.

use crate::{
    Error,
    cache::{Cache, ResolvedImage},
};
use carapace::Assemble as _;
use pichi_artifact::{Layer, ScuteAnnotations, ScuteDescriptor};
use pichi_import::verity::{VerityFormat as _, VerityParams};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

const BLOCK: u64 = 4096;

#[derive(Debug)]
pub struct Scute {
    pub descriptor: ScuteDescriptor,
    pub root: [u8; 32],
    pub parent: [u8; 32],
    data: PathBuf,
    hash: PathBuf,
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Invalid(message.into())
}
fn format_error(error: impl std::fmt::Display) -> Error {
    invalid(format!("scute format: {error}"))
}

pub(crate) fn new_file(path: &Path) -> Result<File, Error> {
    Ok(OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(path)?)
}

pub fn digest_file(path: &Path) -> Result<(String, u64), Error> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let size = std::io::copy(&mut file, &mut digest)?;
    Ok((format!("sha256:{:x}", digest.finalize()), size))
}

impl Scute {
    /// Convert an independently produced raw filesystem using pichi import's
    /// streaming encoder. The tool carapace contributes no ancestor layer.
    pub fn import(raw: &Path, directory: &Path) -> Result<Self, Error> {
        std::fs::create_dir(directory)?;
        let data = directory.join("data");
        let mut cow = new_file(&data)?;
        let source = File::open(raw)?;
        let meta = pichi_import::cow::write_streaming_sparse(
            &source,
            source.metadata()?.len(),
            &mut cow,
            8,
        )
        .map_err(format_error)?;
        cow.set_len(meta.total_bytes)?;
        drop(cow);
        Self::seal(data, directory.join("hash"), [0; 32])
    }

    /// Seal a completed and detached writable COW. Callers must stop every
    /// writer and release its device before invoking this method.
    pub fn seal(data: PathBuf, hash: PathBuf, parent: [u8; 32]) -> Result<Self, Error> {
        let (digest, size) = digest_file(&data)?;
        if size == 0 || size % BLOCK != 0 {
            return Err(invalid("COW must contain whole 4096-byte blocks"));
        }
        let parameters = params(parent.to_vec());
        let root = parameters
            .header(size)?
            .format_padded(File::open(&data)?, new_file(&hash)?)?;
        Ok(Self {
            descriptor: ScuteDescriptor {
                digest,
                size,
                annotations: ScuteAnnotations {
                    data_blocks: Some(size / BLOCK),
                    salt: hex::encode(parent),
                },
            },
            root,
            parent,
            data,
            hash,
        })
    }

    fn stage(
        cache: &Cache,
        descriptor: &ScuteDescriptor,
        compressed: bool,
        directory: &Path,
    ) -> Result<Self, Error> {
        std::fs::create_dir(directory)?;
        let source = directory.join("source");
        cache.copy_blob(&descriptor.digest, descriptor.size, &mut new_file(&source)?)?;
        let data = if compressed {
            let path = directory.join("data");
            let mut decoder = zstd::stream::read::Decoder::new(File::open(&source)?)?;
            std::io::copy(&mut decoder, &mut new_file(&path)?)?;
            path
        } else {
            source
        };
        let salt = hex::decode(&descriptor.annotations.salt).map_err(format_error)?;
        let parent = salt
            .get(..32)
            .ok_or_else(|| invalid("scute salt is missing its parent root"))?
            .try_into()
            .map_err(format_error)?;
        let parameters = params(salt);
        let size = File::open(&data)?.metadata()?.len();
        let header = match descriptor.annotations.data_blocks {
            Some(blocks) => parameters.header_for_blocks(blocks)?,
            None => parameters.header(size)?,
        };
        let hash = directory.join("hash");
        let root = header.format_padded(File::open(&data)?, new_file(&hash)?)?;
        Ok(Self {
            descriptor: descriptor.clone(),
            root,
            parent,
            data,
            hash,
        })
    }

    fn files(&self) -> Result<carapace::Scute, Error> {
        Ok(carapace::Scute {
            data: File::open(&self.data)?,
            hash: File::open(&self.hash)?,
        })
    }

    pub fn data_path(&self) -> &Path {
        &self.data
    }
}

fn params(salt: Vec<u8>) -> VerityParams {
    VerityParams {
        data_block_size: 4096,
        hash_block_size: 4096,
        salt,
        uuid: [0; 16],
    }
}

/// Fully verified, privately staged scutes in base-to-top order.
#[derive(Debug)]
pub struct Chain {
    pub scutes: Vec<Scute>,
}

impl Chain {
    pub fn stage(cache: &Cache, image: &ResolvedImage, directory: &Path) -> Result<Self, Error> {
        // carapace's allowed format is fixed. Never reconstruct unsupported
        // parameters using defaults and accidentally authenticate a different tree.
        for (key, expected) in [
            ("algo", "sha256"),
            ("data-block-size", "4096"),
            ("hash-block-size", "4096"),
            ("version", "1"),
            ("hash-type", "1"),
        ] {
            if image
                .manifest
                .annotations
                .get(&format!("dev.pichi.carapace.verity.{key}"))
                .map(String::as_str)
                != Some(expected)
            {
                return Err(invalid(format!("unsupported carapace verity {key}")));
            }
        }
        let mut root = image
            .manifest
            .carapace_verity_hash()
            .ok_or_else(|| invalid("image has no carapace root"))?;
        std::fs::create_dir(directory)?;
        let mut candidates = BTreeMap::new();
        for (index, layer) in image.manifest.layers.iter().enumerate() {
            let (Layer::Scute(descriptor) | Layer::ScuteZstd(descriptor)) = layer else {
                continue;
            };
            let scute = Scute::stage(
                cache,
                descriptor,
                layer.is_zstd_variant(),
                &directory.join(index.to_string()),
            )?;
            if candidates.insert(scute.root, scute).is_some() {
                return Err(invalid("duplicate scute roots"));
            }
        }
        let mut scutes = Vec::new();
        while root != [0; 32] {
            let scute = candidates
                .remove(&root)
                .ok_or_else(|| invalid("scute chain does not match the authenticated root"))?;
            root = scute.parent;
            scutes.push(scute);
        }
        if scutes.is_empty() || !candidates.is_empty() {
            return Err(invalid("image contains disconnected scutes"));
        }
        scutes.reverse();
        Ok(Self { scutes })
    }

    pub fn root(&self) -> Result<[u8; 32], Error> {
        self.scutes
            .last()
            .map(|s| s.root)
            .ok_or_else(|| invalid("empty carapace"))
    }

    pub fn assemble(&self) -> Result<File, Error> {
        let chain = carapace::Chain {
            root: self.root()?.into(),
            scutes: self
                .scutes
                .iter()
                .map(Scute::files)
                .collect::<Result<_, _>>()?,
        };
        Ok(chain.assemble()?)
    }
}

/// Remove unused sparse capacity from a kernel-written COW after detachment.
/// Retain every data chunk and the terminating metadata area, including the
/// extra empty area required when the previous area was completely filled.
pub(crate) fn trim_cow(file: &mut File) -> Result<(), Error> {
    file.seek(SeekFrom::Start(0))?;
    let mut header = [0; 16];
    file.read_exact(&mut header)?;
    if header != [0x53, 0x6e, 0x41, 0x70, 1, 0, 0, 0, 1, 0, 0, 0, 8, 0, 0, 0] {
        return Err(invalid("invalid or overflowed writable snapshot"));
    }
    let capacity = file.metadata()?.len() / BLOCK;
    let mut area = 1_u64;
    let mut end = 2_u64;
    let mut metadata = [0; 4096];
    loop {
        if area >= capacity {
            return Err(invalid("COW has no terminating metadata area"));
        }
        file.seek(SeekFrom::Start(area * BLOCK))?;
        file.read_exact(&mut metadata)?;
        end = end.max(area + 1);
        for entry in metadata.as_chunks::<16>().0 {
            let chunk = u64::from_le_bytes(entry[8..16].try_into().map_err(format_error)?);
            if chunk == 0 {
                file.set_len(end * BLOCK)?;
                return Ok(());
            }
            if chunk >= capacity || chunk < 2 || (chunk - 1) % 257 == 0 {
                return Err(invalid("COW exception points outside its data area"));
            }
            end = end.max(chunk + 1);
        }
        area += 257;
    }
}
