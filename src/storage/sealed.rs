// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::Scratch;
use crate::artifact::{ScuteAnnotations, ScuteDescriptor};
use devmap_core::{BlockSize, Geometry};
use devmap_snapshot::{Exception, Header};
use devmap_verity::{Algorithm, Format as _, HashType, Header as VerityHeader, Parameters, Salt};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    num::NonZero,
    os::unix::fs::FileExt,
};

/// Immutable local scute bytes and generated verity tree. No writable scratch
/// authority escapes. This proves sealing, not command success or publication.
///
#[derive(Debug)]
pub(crate) struct SealedScute<'workspace> {
    data: Scratch<'workspace>,
    hash: Scratch<'workspace>,
    descriptor: ScuteDescriptor,
    root: [u8; 32],
    parent: [u8; 32],
}
impl SealedScute<'_> {
    pub(crate) fn files(&self) -> (&File, &File) {
        (self.data.file(), self.hash.file())
    }
    pub(crate) fn descriptor(&self) -> &ScuteDescriptor {
        &self.descriptor
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn parent(&self) -> &[u8; 32] {
        &self.parent
    }
    pub(crate) fn data_reader(&self) -> ScuteReader<'_> {
        ScuteReader {
            file: self.data.file(),
            position: 0,
            length: self.descriptor.size,
        }
    }
}
/// Read/seek-only view; neither a raw file descriptor nor write access escapes.
#[derive(Debug)]
pub(crate) struct ScuteReader<'a> {
    file: &'a File,
    position: u64,
    length: u64,
}
impl<'a> ScuteReader<'a> {
    pub(crate) fn new(file: &'a File, length: u64) -> Self {
        Self {
            file,
            position: 0,
            length,
        }
    }
}
impl Read for ScuteReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let count = self
            .length
            .saturating_sub(self.position)
            .min(buffer.len() as u64) as usize;
        if count == 0 {
            return Ok(0);
        }
        let read = self.file.read_at(&mut buffer[..count], self.position)?;
        self.position += read as u64;
        Ok(read)
    }
}
impl Seek for ScuteReader<'_> {
    fn seek(&mut self, offset: SeekFrom) -> io::Result<u64> {
        self.position = match offset {
            SeekFrom::Start(position) => Some(position),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
            SeekFrom::End(delta) => self.length.checked_add_signed(delta),
        }
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid scute seek"))?;
        Ok(self.position)
    }
}
impl<'w> Scratch<'w> {
    /// Seal a detached persistent snapshot store. LoopAttachment::detach must
    /// have confirmed unbound state before returning this writable authority.
    /// Only unused trailing capacity is removed; no filesystem image is copied
    /// or converted. Caller separately owns command/finalization success proofs.
    pub(crate) fn seal_snapshot(mut self, parent: [u8; 32]) -> io::Result<SealedScute<'w>> {
        let capacity = self.file.metadata()?.len();
        let extent = SnapshotExtent::read(&mut self.file, capacity)?;
        self.file.set_len(extent.bytes)?;
        self.file.sync_all()?;
        self.file.rewind()?;
        let mut hash = self._workspace.scratch()?;
        let mut input = MeasuredRead {
            input: &mut self.file,
            digest: Sha256::new(),
        };
        let data_blocks = NonZero::new(extent.bytes / 4096).expect("nonempty snapshot extent");
        let block = BlockSize::from_bytes(NonZero::new(4096).expect("nonzero block size"))
            .expect("valid block size");
        // Preserve the parent link and independently randomize this new layer.
        let mut salt = [0; 64];
        salt[..32].copy_from_slice(&parent);
        getrandom::fill(&mut salt[32..]).map_err(io::Error::other)?;
        let header = VerityHeader {
            uuid: [0; 16],
            parameters: Parameters {
                algorithm: Algorithm::Sha256,
                hash_type: HashType::Normal,
                salt: Salt::new(&salt).expect("parent and random suffix fit salt"),
                data: Geometry {
                    size: block,
                    count: data_blocks,
                },
                hash: block,
            },
        };
        let root = header.format(&mut input, &mut hash)?;
        let root = root.as_ref().try_into().expect("SHA-256 root width");
        let digest = format!("sha256:{}", hex::encode(input.digest.finalize()));
        hash.sync_all()?;
        let descriptor = ScuteDescriptor {
            digest,
            size: extent.bytes,
            annotations: ScuteAnnotations {
                data_blocks,
                salt: hex::encode(salt),
            },
        };
        Ok(SealedScute {
            data: self,
            hash,
            root,
            descriptor,
            parent,
        })
    }
}
#[derive(Debug)]
struct MeasuredRead<R> {
    input: R,
    digest: Sha256,
}
impl<R: Read> Read for MeasuredRead<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.input.read(buffer)?;
        self.digest.update(&buffer[..read]);
        Ok(read)
    }
}
/// Application-specific used extent of our kernel-produced 4KiB snapshot store.
/// Wire decoding stays in devmap-snapshot. This is not an independent verifier
/// of every snapshot semantic: the kernel health check precedes detachment.
#[derive(Debug)]
struct SnapshotExtent {
    bytes: u64,
}
impl SnapshotExtent {
    fn read(input: &mut (impl Read + Seek), capacity: u64) -> io::Result<Self> {
        if capacity < 8192 || !capacity.is_multiple_of(4096) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid snapshot capacity",
            ));
        }
        input.rewind()?;
        let mut record = [0; 16];
        input.read_exact(&mut record)?;
        let header = Header::decode(record)?;
        if !header.valid || header.chunk_size.bytes().get() != 4096 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid snapshot header",
            ));
        }
        let chunks = capacity / 4096;
        let stride = header.exceptions_per_chunk() + 1;
        let mut end = 2;
        for index in 0..chunks {
            let area = header.metadata_chunk_index(index)?;
            if area >= chunks {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "snapshot lacks terminating metadata",
                ));
            }
            input.seek(SeekFrom::Start(area * 4096))?;
            let mut metadata = [0; 4096];
            input.read_exact(&mut metadata)?;
            end = end.max(area + 1);
            for record in metadata.as_chunks::<16>().0 {
                let exception = Exception::decode(*record);
                if exception.new_chunk == 0 {
                    return Ok(Self { bytes: end * 4096 });
                }
                if exception.old_chunk >= carapace::ZERO_SIZE_BYTES / 4096
                    || exception.new_chunk < 2
                    || exception.new_chunk >= chunks
                    || (exception.new_chunk - 1).is_multiple_of(stride)
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "snapshot exception outside data geometry",
                    ));
                }
                end = end.max(exception.new_chunk + 1);
            }
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "snapshot metadata exceeds capacity",
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[derive(Debug)]
    struct Store(Vec<u8>);
    impl Store {
        fn new(chunks: usize) -> Self {
            let mut bytes = vec![0; chunks * 4096];
            bytes[..16].copy_from_slice(
                &Header {
                    valid: true,
                    chunk_size: Default::default(),
                }
                .encode(),
            );
            Self(bytes)
        }
        fn entry(&mut self, index: usize, old_chunk: u64, new_chunk: u64) {
            let offset = 4096 + index * 16;
            self.0[offset..offset + 16].copy_from_slice(
                &Exception {
                    old_chunk,
                    new_chunk,
                }
                .encode(),
            );
        }
        fn extent(&self) -> io::Result<u64> {
            SnapshotExtent::read(&mut Cursor::new(&self.0), self.0.len() as u64)
                .map(|extent| extent.bytes)
        }
    }
    #[test]
    fn sealed_snapshot_preserves_bytes_digest_and_tree() {
        use std::io::Write;
        let directory = tempfile::tempdir().unwrap();
        let workspace = super::super::Workspace {
            root: File::open(directory.path()).unwrap(),
        };
        let bytes = Store::new(2).0;
        let mut salts = Vec::new();
        let mut roots = Vec::new();
        for _ in 0..2 {
            let mut scratch = workspace.scratch().unwrap();
            scratch.write_all(&bytes).unwrap();
            // Unused capacity is deliberately excluded from both commitments.
            scratch.set_len(4 * 4096).unwrap();
            let parent = [3; 32];
            let sealed = scratch.seal_snapshot(parent).unwrap();
            let mut data = Vec::new();
            sealed.data_reader().read_to_end(&mut data).unwrap();
            assert_eq!(data, bytes);
            assert_eq!(
                sealed.descriptor().digest,
                format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
            );
            assert_eq!(sealed.descriptor().annotations.data_blocks.get(), 2);
            let mut tree = vec![0; 8192];
            sealed.files().1.read_exact_at(&mut tree, 0).unwrap();
            assert_eq!(sealed.files().1.metadata().unwrap().len(), 8192);
            let header = VerityHeader::decode(tree[..512].try_into().unwrap()).unwrap();
            let salt = header.parameters.salt.as_ref();
            assert_eq!(salt.len(), 64);
            assert_eq!(&salt[..32], parent);
            assert_eq!(sealed.descriptor().annotations.salt, hex::encode(salt));
            assert_eq!(header.parameters.data.count.get(), 2);
            for (index, block) in bytes.chunks_exact(4096).enumerate() {
                let mut hash = Sha256::new();
                hash.update(salt);
                hash.update(block);
                assert_eq!(
                    &tree[4096 + index * 32..4096 + (index + 1) * 32],
                    hash.finalize().as_slice()
                );
            }
            assert!(tree[4160..].iter().all(|byte| *byte == 0));
            let mut hash = Sha256::new();
            hash.update(salt);
            hash.update(&tree[4096..]);
            assert_eq!(sealed.root().as_slice(), hash.finalize().as_slice());
            salts.push(salt.to_vec());
            roots.push(*sealed.root());
        }
        assert_ne!(salts[0][32..], salts[1][32..]);
        assert_ne!(roots[0], roots[1]);
    }
    #[test]
    fn sealing_rejects_incomplete_snapshot_extents() {
        use std::io::Write;
        let directory = tempfile::tempdir().unwrap();
        let workspace = super::super::Workspace {
            root: File::open(directory.path()).unwrap(),
        };
        for length in [0, 4096, 4097, 8191] {
            let mut scratch = workspace.scratch().unwrap();
            scratch.write_all(&Store::new(2).0[..length]).unwrap();
            assert!(scratch.seal_snapshot([0; 32]).is_err());
        }
    }
    #[test]
    fn sealed_outputs_commit_the_final_root() {
        use crate::{cache::Cache, output::AssembledImage, storage::Workspace};
        use std::{io::Write, sync::Arc};

        let directory = tempfile::tempdir().unwrap();
        let workspace = Workspace {
            root: File::open(directory.path()).unwrap(),
        };
        let mut scratch = workspace.scratch().unwrap();
        scratch.write_all(&Store::new(2).0).unwrap();
        let base = Arc::new(scratch.seal_snapshot([0; 32]).unwrap());
        let output = directory.path().join("base");
        let cache = Cache::open(directory.path()).unwrap();
        let selection = crate::pipeline::output::StageOutput {
            root: Some(crate::pipeline::root::Root {
                base: None,
                produced: vec![Arc::clone(&base)],
            }),
            pmi: None,
            bootconfig: None,
            config_requested: false,
        };
        let image = AssembledImage::components(&cache, &selection, &output).unwrap();
        assert_eq!(
            image.image().manifest().carapace_root().unwrap(),
            Some(*base.root())
        );
        image.publish().unwrap();

        let index = std::fs::read(output.join("index.json")).unwrap();
        let digest = crate::artifact::Digest::Sha256(Sha256::digest(&index).into());
        std::fs::write(output.join("blobs/sha256").join(digest.hex()), &index).unwrap();
        let cache = Cache::open(&output).unwrap();
        let image = crate::input::CheckedImage::verify(
            &cache,
            cache.resolve(digest.to_string().parse().unwrap()).unwrap(),
        )
        .unwrap();
        let mut scratch = workspace.scratch().unwrap();
        scratch.write_all(&Store::new(2).0).unwrap();
        let top = Arc::new(scratch.seal_snapshot(*base.root()).unwrap());
        let extended = AssembledImage::components(
            &cache,
            &crate::pipeline::output::StageOutput {
                root: Some(crate::pipeline::root::Root {
                    base: Some(&image),
                    produced: vec![Arc::clone(&top)],
                }),
                pmi: None,
                bootconfig: None,
                config_requested: false,
            },
            &directory.path().join("extended"),
        )
        .unwrap();
        let bootstrap = AssembledImage::components(
            &cache,
            &crate::pipeline::output::StageOutput {
                root: Some(crate::pipeline::root::Root {
                    base: None,
                    produced: vec![base, Arc::clone(&top)],
                }),
                pmi: None,
                bootconfig: None,
                config_requested: false,
            },
            &directory.path().join("bootstrap"),
        )
        .unwrap();
        for output in [extended, bootstrap] {
            assert_eq!(
                output.image().manifest().carapace_root().unwrap(),
                Some(*top.root())
            );
            assert_eq!(output.image().manifest().layers.len(), 2);
            assert_eq!(output.image().root(), Some(*top.root()));
            output.discard().unwrap();
        }
    }
    #[test]
    fn readonly_cursors_are_independent_and_allow_eof_seeks() {
        use std::io::Write;
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"layer").unwrap();
        let mut first = ScuteReader {
            file: &file,
            position: 0,
            length: 5,
        };
        let mut second = ScuteReader {
            file: &file,
            position: 0,
            length: 5,
        };
        first.seek(SeekFrom::End(-1)).unwrap();
        let mut byte = [0];
        first.read_exact(&mut byte).unwrap();
        assert_eq!(byte, *b"r");
        second.read_exact(&mut byte).unwrap();
        assert_eq!(byte, *b"l");
        first.seek(SeekFrom::Start(u64::MAX)).unwrap();
        assert_eq!(first.read(&mut byte).unwrap(), 0);
        assert!(second.seek(SeekFrom::Current(-2)).is_err());
    }
    #[test]
    fn trailing_capacity_and_full_metadata_boundary() {
        let mut store = Store::new(270);
        assert_eq!(store.extent().unwrap(), 8192);
        store.entry(0, 12, 7);
        assert_eq!(store.extent().unwrap(), 8 * 4096);
        for index in 0..256 {
            store.entry(index, index as u64, index as u64 + 2);
        }
        assert_eq!(store.extent().unwrap(), 259 * 4096);
        store.0.truncate(258 * 4096);
        assert!(store.extent().is_err());
    }
    #[test]
    fn malformed_records_never_select_an_extent() {
        for new in [1, 258, 300] {
            let mut store = Store::new(270);
            store.entry(0, 0, new);
            assert!(store.extent().is_err());
        }
        let mut store = Store::new(3);
        store.entry(0, carapace::ZERO_SIZE_BYTES / 4096, 2);
        assert!(store.extent().is_err());
        store.0[4..8].fill(0);
        assert!(store.extent().is_err());
    }
}
