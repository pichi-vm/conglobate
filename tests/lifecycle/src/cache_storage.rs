// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    cache::{Cache, FromIndex as _},
    storage::ReadOnlyChain,
};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

#[derive(Debug)]
pub(super) struct CacheStorage;
impl CacheStorage {
    fn node(path: &Path, device: u64) -> std::io::Result<()> {
        rustix::fs::mknodat(
            rustix::fs::CWD,
            path,
            rustix::fs::FileType::BlockDevice,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            device,
        )?;
        Ok(())
    }

    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let cache = Cache::open(Path::new("/cache-fixture"))?;
        let image = cache.resolve(fs::read_to_string("/cache-fixture/aligned.pin")?.parse()?)?;
        let baseline = super::chain_storage::ChainStorage::mappings()?;
        let discovery = carapace::Chain::from_index(&cache, &image)?;
        assert_eq!(super::chain_storage::ChainStorage::mappings()?, baseline);
        // Reopen the same retained inodes as a malicious cache owner.
        use std::os::fd::AsRawFd as _;
        let raw_node = format!("/proc/self/fd/{}", discovery.scutes[0].data.as_raw_fd());
        let hash_node = format!("/proc/self/fd/{}", discovery.scutes[0].hash.as_raw_fd());
        let mut hash = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&hash_node)?;
        let mut corruptor = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&raw_node)?;
        drop(discovery);
        let node = Path::new("/dev/conglobate-verity-probe");

        // Model a malicious host: corrupt a payload before activation. The
        // header and used metadata remain valid, so only reading it must fail.
        corruptor.seek(SeekFrom::Start(12288))?;
        let mut original = [0; 4096];
        corruptor.read_exact(&mut original)?;
        corruptor.seek(SeekFrom::Start(12288))?;
        corruptor.write_all(&[1; 4096])?;
        corruptor.sync_all()?;
        let verified =
            ReadOnlyChain::assemble(Some(carapace::Chain::from_index(&cache, &image)?), &[])?;
        let id = verified.id();
        Self::node(node, rustix::fs::makedev(id.major(), id.minor()))?;
        let mut holder = fs::File::open(node)?;
        let mut first = [0; 4096];
        holder.read_exact(&mut first)?;
        assert_eq!(first, [42; 4096]);
        assert_eq!(
            holder.read_exact(&mut first).unwrap_err().raw_os_error(),
            Some(5)
        );
        drop(holder);
        verified.close()?;
        fs::remove_file(node)?;
        corruptor.seek(SeekFrom::Start(12288))?;
        corruptor.write_all(&original)?;
        corruptor.sync_all()?;
        drop(corruptor);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while super::chain_storage::ChainStorage::mappings()? != baseline {
            assert!(
                std::time::Instant::now() < deadline,
                "input mappings leaked"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        println!("INPUT_CORRUPTION_IS_LAZY_PASS");
        println!("VERITY_CACHE_CORRUPTION_READ_REJECTED_PASS");

        // Header restrictions still apply even though manifest hints are unused.
        for (offset, changed) in [
            (8, 2u32.to_le_bytes().to_vec()),
            (12, 0u32.to_le_bytes().to_vec()),
            (72, u64::MAX.to_le_bytes().to_vec()),
        ] {
            let discovery = carapace::Chain::from_index(&cache, &image)?;
            let mut original = vec![0; changed.len()];
            hash.seek(SeekFrom::Start(offset))?;
            hash.read_exact(&mut original)?;
            hash.seek(SeekFrom::Start(offset))?;
            hash.write_all(&changed)?;
            hash.sync_all()?;
            assert!(carapace::Chain::from_index(&cache, &image).is_err());
            assert!(ReadOnlyChain::assemble(Some(discovery), &[]).is_err());
            hash.seek(SeekFrom::Start(offset))?;
            hash.write_all(&original)?;
            hash.sync_all()?;
        }
        drop(hash);
        let missing =
            cache.resolve(fs::read_to_string("/cache-fixture/missing-root.pin")?.parse()?)?;
        assert!(carapace::Chain::from_index(&cache, &missing).is_err());
        println!("MISSING_ROOT_DISCOVERY_REJECTED_PASS");

        for name in ["aligned", "partial", "compressed", "header-hints"] {
            let pin = fs::read_to_string(format!("/cache-fixture/{name}.pin"))?;
            let image = cache.resolve(pin.parse()?)?;
            // Compressed inputs execute entirely from their prepared files.
            for layer in &image.manifest().layers {
                if !matches!(layer, conglobate::artifact::Layer::ScuteZstd(_)) {
                    continue;
                }
                let path = format!(
                    "/cache-fixture/blobs/sha256/{}",
                    layer.digest_str().strip_prefix("sha256:").unwrap()
                );
                if Path::new(&path).exists() {
                    fs::remove_file(path)?;
                }
            }
            cache.verify_metadata(&image)?;
            let discovery = carapace::Chain::from_index(&cache, &image)?;
            let verified = ReadOnlyChain::assemble(Some(discovery), &[])?;
            let id = verified.id();
            Self::node(node, rustix::fs::makedev(id.major(), id.minor()))?;
            if let Ok(mut writer) = fs::OpenOptions::new().write(true).open(node) {
                assert!(writer.write_all(&[0; 4096]).is_err());
            }
            let mut holder = fs::File::open(node)?;
            let mut data = [0; 8192];
            holder.read_exact(&mut data)?;
            let used = if name == "partial" || name == "compressed" {
                4097
            } else {
                8192
            };
            assert!(data[..used].iter().all(|byte| *byte == 42));
            assert!(data[used..].iter().all(|byte| *byte == 0));
            if name == "compressed" {
                assert_eq!(
                    verified.close().unwrap_err().kind(),
                    std::io::ErrorKind::ResourceBusy
                );
                holder.rewind()?;
                let mut byte = [0];
                holder.read_exact(&mut byte)?;
                assert_eq!(byte, [42]);
                println!("VERITY_TERMINAL_CLOSE_RETAINS_LIVE_HOLDER_PASS");
                drop(holder);
            } else {
                drop(holder);
                verified.close()?;
            }
            fs::remove_file(node)?;
        }
        println!("PREPARED_INPUTS_USE_VALIDATED_HEADERS_PASS");
        println!("OWNED_VERITY_READ_AND_CLEANUP_PASS");
        Ok(())
    }
}
