// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Scratch, SnapshotOrigin};
use crate::artifact::{ScuteAnnotations, ScuteDescriptor};
use crate::output::MeasuredWrite;
use devmap_core::{BlockSize, Geometry};
use devmap_snapshot::{Exception, Header};
use devmap_verity::{Algorithm, Format as _, HashType, Header as VerityHeader, Parameters, Salt};
use sha2::Digest;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    num::NonZero,
    os::unix::fs::FileExt,
};

/// Output-backed prepared files plus identities retained inside the VM.
/// Storage remains mutable outside the VM; reuse must go through verity.
/// This proves sealing, not command success or publication.
///
#[derive(Debug)]
pub(crate) struct SealedScute {
    data: tempfile::NamedTempFile,
    hash: tempfile::NamedTempFile,
    blob_name: std::sync::Mutex<Option<tempfile::TempPath>>,
    descriptor: ScuteDescriptor,
    root: [u8; 32],
    parent: [u8; 32],
    // Temp paths use this descriptor; drop it after all temporary paths.
    _directory: File,
}
impl SealedScute {
    pub(crate) fn files(&self) -> (&File, &File) {
        (self.data.as_file(), self.hash.as_file())
    }
    pub(crate) fn install_blob(&self, destination: &std::path::Path) -> io::Result<()> {
        let mut name = self
            .blob_name
            .lock()
            .map_err(|_| io::Error::other("prepared scute ownership poisoned"))?;
        let pending = name
            .take()
            .ok_or_else(|| io::Error::other("encoded scute blob already installed"))?;
        match pending.persist(destination) {
            Ok(()) => Ok(()),
            Err(error) => {
                *name = Some(error.path);
                Err(error.error)
            }
        }
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
    #[cfg(test)]
    pub(crate) fn data_reader(&self) -> ScuteReader<'_> {
        ScuteReader {
            file: self.data.as_file(),
            position: 0,
            length: self.descriptor.annotations.data_blocks.get() * 4096,
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
    /// Compare against the retained, read-only origin, then stream final-order
    /// bytes into verity, the prepared COW, and zstd together. In-place exception
    /// planning supplies final geometry without a compacted-file intermediate.
    /// Caller separately owns command/finalization success proofs.
    pub(crate) fn seal_snapshot<'a>(
        self,
        parent: [u8; 32],
        origin: impl Into<SnapshotOrigin<'a>>,
        directory: File,
    ) -> io::Result<SealedScute> {
        let origin = origin.into().open_read_only()?;
        self.seal_with_reader(directory, parent, |offset, bytes| {
            origin.read_exact_at(bytes, offset)
        })
    }

    fn seal_with_reader(
        mut self,
        directory: File,
        parent: [u8; 32],
        read_origin: impl FnMut(u64, &mut [u8; 4096]) -> io::Result<()>,
    ) -> io::Result<SealedScute> {
        crate::output::report_memory_phase("scute-start");
        let capacity = self.file.metadata()?.len();
        let extent = SnapshotExtent::read(&mut self.file, capacity)?;
        self.file.set_len(extent.bytes)?;
        let compacted = super::compact::CompactSnapshot::new(&mut self.file, read_origin)?;
        crate::output::report_memory_phase("scute-planned");
        let bytes = compacted.size_bytes();
        use std::os::fd::AsRawFd as _;
        let path = format!("/proc/self/fd/{}", directory.as_raw_fd());
        let mut packed = tempfile::Builder::new()
            .prefix(".scute-data-")
            .tempfile_in(&path)?;
        let mut hash = tempfile::Builder::new()
            .prefix(".scute-tree-")
            .tempfile_in(&path)?;
        let blob = tempfile::Builder::new()
            .prefix(".scute-blob-")
            .tempfile_in(&path)?;
        let mut encoder = zstd::stream::write::Encoder::new(MeasuredWrite::new(blob), 3)?;
        let data_blocks = NonZero::new(bytes / 4096).expect("nonempty snapshot extent");
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
        let root = {
            let mut input = MeasuredRead {
                input: compacted,
                data: &mut packed,
                encoded: &mut encoder,
            };
            header.format(&mut input, &mut hash)?
        };
        let root = root.as_ref().try_into().expect("SHA-256 root width");
        crate::output::report_memory_phase("scute-formatted");
        // The frame trailer contributes to encoded identity and size too.
        let MeasuredWrite {
            output: blob,
            digest,
            size,
        } = encoder.finish()?;
        packed.as_file().sync_all()?;
        hash.as_file().sync_all()?;
        blob.as_file().sync_all()?;
        // Working reads use the plain COW; completed transport need not stay cached.
        let _ = rustix::fs::fadvise(blob.as_file(), 0, None, rustix::fs::Advice::DontNeed);
        crate::output::report_memory_phase("scute-finished");
        let descriptor = ScuteDescriptor {
            digest: format!("sha256:{}", hex::encode(digest.finalize())),
            size,
            annotations: ScuteAnnotations {
                data_blocks,
                salt: hex::encode(salt),
            },
        };
        Ok(SealedScute {
            data: packed,
            hash,
            blob_name: std::sync::Mutex::new(Some(blob.into_temp_path())),
            root,
            descriptor,
            parent,
            _directory: directory,
        })
    }
}
#[derive(Debug)]
struct MeasuredRead<R, W, E> {
    input: R,
    data: W,
    encoded: E,
}
impl<R: Read, W: Write, E: Write> Read for MeasuredRead<R, W, E> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.input.read(buffer)?;
        self.data.write_all(&buffer[..read])?;
        self.encoded.write_all(&buffer[..read])?;
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
    use sha2::Sha256;
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
    fn encoded_blob(sealed: &SealedScute, plain: &[u8]) -> Vec<u8> {
        let name = sealed.blob_name.lock().unwrap();
        let blob = std::fs::read(name.as_ref().unwrap()).unwrap();
        assert_eq!(sealed.descriptor().size, blob.len() as u64);
        assert_eq!(
            sealed.descriptor().digest,
            format!("sha256:{}", hex::encode(Sha256::digest(&blob)))
        );
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(blob.as_slice()).unwrap();
        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();
        assert_eq!(decoded, plain);
        blob
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
            let sealed = scratch
                .seal_with_reader(
                    workspace.staging_parent().unwrap(),
                    parent,
                    |_, _| unreachable!(),
                )
                .unwrap();
            let mut data = Vec::new();
            sealed.data_reader().read_to_end(&mut data).unwrap();
            assert_eq!(data, bytes);
            encoded_blob(&sealed, &bytes);
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
    fn sealing_encodes_only_packed_exceptions_and_propagates_origin_failure() {
        use std::io::Write;
        let directory = tempfile::tempdir().unwrap();
        let workspace = super::super::Workspace {
            root: File::open(directory.path()).unwrap(),
        };
        let mut input = Store::new(8);
        input.entry(0, 1, 7);
        input.entry(1, 2, 4);
        input.0[7 * 4096..8 * 4096].fill(9);
        let mut scratch = workspace.scratch().unwrap();
        scratch.write_all(&input.0).unwrap();
        assert!(
            scratch
                .seal_with_reader(workspace.staging_parent().unwrap(), [3; 32], |_, _| Err(
                    io::ErrorKind::PermissionDenied.into()
                ))
                .is_err()
        );
        let mut scratch = workspace.scratch().unwrap();
        scratch.write_all(&input.0).unwrap();
        let sealed = scratch
            .seal_with_reader(workspace.staging_parent().unwrap(), [3; 32], |_, bytes| {
                bytes.fill(9);
                Ok(())
            })
            .unwrap();
        let mut packed = Vec::new();
        sealed.data_reader().read_to_end(&mut packed).unwrap();
        assert_eq!(packed.len(), 3 * 4096);
        let entry = Exception::decode(packed[4096..4112].try_into().unwrap());
        assert_eq!((entry.old_chunk, entry.new_chunk), (2, 2));
        assert!(packed[4112..].iter().all(|byte| *byte == 0));
        encoded_blob(&sealed, &packed);
        assert_eq!(sealed.descriptor().annotations.data_blocks.get(), 3);
        let mut header = [0; 512];
        sealed.files().1.read_exact_at(&mut header, 0).unwrap();
        assert_eq!(
            VerityHeader::decode(header)
                .unwrap()
                .parameters
                .data
                .count
                .get(),
            3
        );
    }
    #[test]
    fn installing_encoded_blob_transfers_only_its_name() {
        let directory = tempfile::tempdir().unwrap();
        let workspace = super::super::Workspace {
            root: File::open(directory.path()).unwrap(),
        };
        let mut scratch = workspace.scratch().unwrap();
        scratch.write_all(&Store::new(2).0).unwrap();
        let sealed = scratch
            .seal_with_reader(
                workspace.staging_parent().unwrap(),
                [0; 32],
                |_, _| unreachable!(),
            )
            .unwrap();
        let encoded = encoded_blob(&sealed, &Store::new(2).0);
        let destination = directory.path().join("installed");
        std::fs::create_dir(&destination).unwrap();
        // A failed rename retains ownership, so it can be retried safely.
        assert!(sealed.install_blob(&destination).is_err());
        std::fs::remove_dir(&destination).unwrap();
        sealed.install_blob(&destination).unwrap();
        assert!(sealed.install_blob(&destination).is_err());
        let mut bytes = Vec::new();
        sealed.data_reader().read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, Store::new(2).0);
        drop(sealed);
        assert_eq!(std::fs::read(&destination).unwrap(), encoded);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    #[test]
    fn encoded_stream_includes_trailer_and_propagates_sink_errors() {
        #[derive(Debug)]
        struct ShortWrites(Vec<u8>);
        impl Write for ShortWrites {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                let count = bytes.len().min(3);
                self.0.extend_from_slice(&bytes[..count]);
                Ok(count)
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        #[derive(Debug)]
        struct Full;
        impl Write for Full {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::StorageFull.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut encoder =
            zstd::stream::write::Encoder::new(MeasuredWrite::new(ShortWrites(Vec::new())), 3)
                .unwrap();
        encoder.write_all(&[0xa5; 8192]).unwrap();
        let encoded = encoder.finish().unwrap();
        assert_eq!(encoded.size, encoded.output.0.len() as u64);
        assert_eq!(
            encoded.digest.finalize().as_slice(),
            Sha256::digest(&encoded.output.0).as_slice()
        );
        let mut decoder =
            ruzstd::decoding::StreamingDecoder::new(encoded.output.0.as_slice()).unwrap();
        let mut decoded = Vec::new();
        decoder.read_to_end(&mut decoded).unwrap();
        assert_eq!(decoded, [0xa5; 8192]);

        let encoder = zstd::stream::write::Encoder::new(MeasuredWrite::new(Full), 3).unwrap();
        assert_eq!(
            encoder.finish().unwrap_err().kind(),
            io::ErrorKind::StorageFull
        );
        let mut encoder = zstd::stream::write::Encoder::new(MeasuredWrite::new(Full), 3).unwrap();
        // Incompressible input forces output during write, before finish.
        let mut input = vec![0; 1024 * 1024];
        for (index, block) in input.chunks_exact_mut(32).enumerate() {
            block.copy_from_slice(Sha256::digest((index as u64).to_le_bytes()).as_slice());
        }
        assert_eq!(
            encoder.write_all(&input).unwrap_err().kind(),
            io::ErrorKind::StorageFull
        );
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
            assert!(
                scratch
                    .seal_with_reader(
                        workspace.staging_parent().unwrap(),
                        [0; 32],
                        |_, _| unreachable!()
                    )
                    .is_err()
            );
        }
    }
    #[test]
    #[ignore = "requires root, loop devices and device mapper"]
    fn output_backed_reuse_authenticates_data_metadata_and_tree() {
        use crate::{output::StagedDirectory, storage::ReadOnlyChain};
        use std::{io::Write, sync::Arc};
        for corruption in 0..5 {
            let directory = tempfile::tempdir().unwrap();
            let workspace = super::super::Workspace {
                root: File::open(directory.path()).unwrap(),
            };
            let staged = StagedDirectory::create(&directory.path().join("output")).unwrap();
            let seal = |value| {
                let mut store = Store::new(3);
                store.entry(0, 0, 2);
                store.0[8192..].fill(value);
                let mut scratch = workspace.scratch().unwrap();
                scratch.write_all(&store.0).unwrap();
                Arc::new(
                    scratch
                        .seal_with_reader(staged.blob_directory().unwrap(), [0; 32], |_, bytes| {
                            bytes.fill(0);
                            Ok(())
                        })
                        .unwrap(),
                )
            };
            let sealed = seal(0xa5);
            match corruption {
                1..=3 => {
                    let (file, offset) = match corruption {
                        1 => (sealed.files().0, 8192),
                        2 => (sealed.files().0, 4096),
                        _ => (sealed.files().1, 4096),
                    };
                    let mut byte = [0];
                    file.read_exact_at(&mut byte, offset).unwrap();
                    byte[0] ^= 0x80;
                    file.write_all_at(&byte, offset).unwrap();
                }
                4 => {
                    let replacement = seal(0x5a);
                    for (source, target) in [
                        (replacement.files().0, sealed.files().0),
                        (replacement.files().1, sealed.files().1),
                    ] {
                        let mut bytes = vec![0; source.metadata().unwrap().len() as usize];
                        source.read_exact_at(&mut bytes, 0).unwrap();
                        target.write_all_at(&bytes, 0).unwrap();
                    }
                }
                _ => {}
            }
            let mut bytes = [0; 4096];
            match ReadOnlyChain::assemble(None, &[Arc::clone(&sealed)]) {
                Ok(chain) => {
                    let read = (|| -> io::Result<()> {
                        let source = SnapshotOrigin::from(&chain).open_read_only()?;
                        source.read_exact_at(&mut bytes, 0)
                    })();
                    chain.close().unwrap();
                    if corruption == 0 {
                        read.unwrap();
                        assert_eq!(bytes, [0xa5; 4096]);
                    } else {
                        assert!(
                            read.is_err(),
                            "modified output must fail authenticated reuse"
                        );
                    }
                }
                Err(error) => assert_ne!(corruption, 0, "valid output-backed chain: {error}"),
            }
        }
    }

    #[test]
    fn sealed_outputs_commit_the_final_root() {
        use crate::{
            cache::Cache,
            output::{AssembledImage, StagedDirectory},
        };
        use std::{io::Write, sync::Arc};
        let directory = tempfile::tempdir().unwrap();
        let workspace = super::super::Workspace {
            root: File::open(directory.path()).unwrap(),
        };
        let seal = |staged: &StagedDirectory, parent| {
            let mut scratch = workspace.scratch().unwrap();
            scratch.write_all(&Store::new(2).0).unwrap();
            Arc::new(
                scratch
                    .seal_with_reader(
                        staged.blob_directory().unwrap(),
                        parent,
                        |_, _| unreachable!(),
                    )
                    .unwrap(),
            )
        };
        let output = directory.path().join("base");
        let staged = StagedDirectory::create(&output).unwrap();
        let base = seal(&staged, [0; 32]);
        let base_root = *base.root();
        let selection = crate::pipeline::output::StageOutput {
            root: Some(crate::pipeline::root::Root {
                base: None,
                produced: vec![base],
            }),
            pmi: None,
            bootconfig: None,
        };
        let cache = Cache::open(directory.path()).unwrap();
        let requirements = serde_json::from_str(r#"{"memory":{"required":1024}}"#).unwrap();
        let image =
            AssembledImage::in_staging(&cache, &selection, Some(&requirements), staged).unwrap();
        let config = image.image().manifest().config.clone();
        assert_eq!(
            image.image().manifest().carapace_root().unwrap(),
            Some(base_root)
        );
        drop(selection);
        image.publish().unwrap();
        assert_eq!(
            std::fs::read(
                output.join("blobs/sha256").join(
                    config
                        .digest
                        .parse::<crate::artifact::Digest>()
                        .unwrap()
                        .hex()
                )
            )
            .unwrap(),
            br#"{"version":1,"requirements":{"memory":{"required":1024}}}"#
        );
        let index = std::fs::read(output.join("index.json")).unwrap();
        let digest = crate::artifact::Digest::Sha256(Sha256::digest(&index).into());
        std::fs::write(output.join("blobs/sha256").join(digest.hex()), &index).unwrap();
        let cache = Cache::open(&output).unwrap();
        let base_image = crate::input::CheckedImage::verify(
            &cache,
            cache.resolve(digest.to_string().parse().unwrap()).unwrap(),
        )
        .unwrap();
        for bootstrap in [false, true] {
            let staged = StagedDirectory::create(&directory.path().join(if bootstrap {
                "bootstrap"
            } else {
                "extended"
            }))
            .unwrap();
            let (base, produced) = if bootstrap {
                let base = seal(&staged, [0; 32]);
                let top = seal(&staged, *base.root());
                (None, vec![base, top])
            } else {
                (Some(&base_image), vec![seal(&staged, base_root)])
            };
            let expected_root = *produced.last().unwrap().root();
            let selection = crate::pipeline::output::StageOutput {
                root: Some(crate::pipeline::root::Root { base, produced }),
                pmi: None,
                bootconfig: None,
            };
            let assembled = AssembledImage::in_staging(&cache, &selection, None, staged).unwrap();
            assert_eq!(
                assembled.image().manifest().carapace_root().unwrap(),
                Some(expected_root)
            );
            assert_eq!(assembled.image().root(), Some(expected_root));
            assert_eq!(assembled.image().manifest().layers.len(), 2);
            if bootstrap {
                assert!(assembled.image().manifest().config.is_empty());
            } else {
                assert_eq!(assembled.image().manifest().config, config);
            }
            drop(selection);
            assembled.discard().unwrap();
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
