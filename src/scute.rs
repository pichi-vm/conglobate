// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Private scute staging, chain verification, and ecosystem format adapters.

use crate::{
    Error,
    cache::{Cache, ResolvedImage},
};
use carapace::{
    Assemble as _,
    image::{Plan as _, Planned},
};
use devmap_core::{BlockSize, Geometry};
use devmap_verity::{Algorithm, Format as _, HashType, Header, Parameters, Salt};
use pichi_artifact::{Layer, ScuteAnnotations, ScuteDescriptor};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    num::NonZero,
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
    /// Plan an independent image, then stream its COW into storage and verity.
    /// The tool carapace contributes no ancestor layer.
    pub fn import(raw: &Path, directory: &Path) -> Result<Self, Error> {
        let planned = Planned::plan(File::open(raw)?)?;
        let size = planned.size();
        std::fs::create_dir(directory)?;
        let data = directory.join("data");
        let hash = directory.join("hash");
        let mut stream = Capture {
            input: planned,
            output: new_file(&data)?,
            digest: Sha256::new(),
        };
        let root = format_exact(
            &header(size / BLOCK, &[0; 32])?,
            &mut stream,
            new_file(&hash)?,
        )?;
        stream.output.flush()?;
        Ok(Self {
            descriptor: ScuteDescriptor {
                digest: format!("sha256:{:x}", stream.digest.finalize()),
                size,
                annotations: ScuteAnnotations {
                    data_blocks: Some(size / BLOCK),
                    salt: hex::encode([0; 32]),
                },
            },
            root,
            parent: [0; 32],
            data,
            hash,
        })
    }

    /// Seal a completed and detached writable COW. Callers must stop every
    /// writer and release its device before invoking this method.
    pub fn seal(data: PathBuf, hash: PathBuf, parent: [u8; 32]) -> Result<Self, Error> {
        let (digest, size) = digest_file(&data)?;
        if size == 0 || size % BLOCK != 0 {
            return Err(invalid("COW must contain whole 4096-byte blocks"));
        }
        let root = format_exact(
            &header(size / BLOCK, &parent)?,
            File::open(&data)?,
            new_file(&hash)?,
        )?;
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
        let size = File::open(&data)?.metadata()?.len();
        let blocks = size.div_ceil(BLOCK);
        if descriptor
            .annotations
            .data_blocks
            .is_some_and(|declared| declared != blocks)
        {
            return Err(invalid("scute length disagrees with declared data_blocks"));
        }
        let header = header(blocks, &salt)?;
        let padding = header.parameters.data_size()? - size;
        let input = File::open(&data)?.chain(io::repeat(0).take(padding));
        let hash = directory.join("hash");
        let root = format_exact(&header, input, new_file(&hash)?)?;
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

// Record the exact emitted bytes while verity consumes the planned reader.
// No reread of the generated COW is needed to calculate either digest.
struct Capture<R> {
    input: R,
    output: File,
    digest: Sha256,
}

impl<R: Read> Read for Capture<R> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        let count = self.input.read(bytes)?;
        self.output.write_all(&bytes[..count])?;
        self.digest.update(&bytes[..count]);
        Ok(count)
    }
}

fn header(blocks: u64, salt: &[u8]) -> Result<Header, Error> {
    let block = NonZero::new(BLOCK as u32)
        .and_then(BlockSize::from_bytes)
        .ok_or_else(|| invalid("invalid scute block size"))?;
    Ok(Header {
        uuid: [0; 16],
        parameters: Parameters {
            algorithm: Algorithm::Sha256,
            hash_type: HashType::Normal,
            salt: Salt::new(salt).ok_or_else(|| invalid("invalid scute salt"))?,
            data: Geometry {
                size: block,
                count: NonZero::new(blocks).ok_or_else(|| invalid("empty scute data"))?,
            },
            hash: block,
        },
    })
}

// The formatter intentionally consumes only its declared extent. Our import and
// cache contracts require EOF as well, including propagation of late read errors.
fn format_exact(
    header: &Header,
    mut input: impl Read,
    hash: impl Write + Seek,
) -> Result<[u8; 32], Error> {
    let root = header.format(&mut input, hash)?;
    if input.read(&mut [0])? != 0 {
        return Err(invalid("scute data exceeds declared extent"));
    }
    root.as_ref().try_into().map_err(format_error)
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Failure;
    impl Read for Failure {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("late input error"))
        }
    }

    #[test]
    fn verity_requires_exact_input_and_reports_late_read_errors() {
        let header = header(1, &[0; 32]).unwrap();
        for length in [4095, 4097] {
            assert!(
                format_exact(
                    &header,
                    vec![0; length].as_slice(),
                    io::Cursor::new(Vec::new())
                )
                .is_err()
            );
        }
        let input = io::Cursor::new(vec![0; 4096]).chain(Failure);
        let error = format_exact(&header, input, io::Cursor::new(Vec::new())).unwrap_err();
        assert!(error.to_string().contains("late input error"));
    }
}
