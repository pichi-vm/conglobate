// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Seek, SeekFrom, Write},
    num::NonZeroU64,
    path::Path,
};

/// Synthetic layer composition and recovery, executed only in the owned VM.
#[derive(Debug)]
pub(super) struct ChainStorage;
impl ChainStorage {
    fn mappings() -> std::io::Result<BTreeSet<std::ffi::OsString>> {
        fs::read_dir("/sys/block")?
            .filter_map(|entry| match entry {
                Ok(entry) if entry.file_name().as_encoded_bytes().starts_with(b"dm-") => {
                    Some(Ok(entry.file_name()))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    fn loop_detachment(
        workspace: &conglobate::storage::Workspace,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut backing = workspace.scratch()?;
        backing.set_len(4096)?;
        let mut attached = backing.attach().map_err(|failure| failure.to_string())?;
        let number = attached.number()?;
        attached.write_all(&[0x5a; 4096])?;
        let mut holder = fs::File::open(format!("/dev/loop{number}"))?;
        let failure = attached.detach().unwrap_err();
        assert_eq!(failure.kind(), std::io::ErrorKind::ResourceBusy);
        let mut bytes = [0; 4096];
        holder.read_exact(&mut bytes)?;
        assert_eq!(bytes, [0x5a; 4096]);
        drop(holder);
        println!("LOOP_DEFERRED_DETACH_WRITEBACK_PASS");
        Ok(())
    }

    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let baseline = Self::mappings()?;
        let workspace = conglobate::storage::Workspace::create()?;
        Self::loop_detachment(&workspace)?;
        let cache = conglobate::cache::Cache::open(Path::new("/cache-fixture"))?;
        for name in ["invalid-header", "chain"] {
            let pin = fs::read_to_string(format!("/cache-fixture/{name}.pin"))?;
            let image = cache.resolve(pin.parse()?)?;
            let chain = cache.verify_chain(&image)?.ok_or("missing chain")?;
            if name == "invalid-header" {
                let failure = chain.assemble(&cache, &workspace).unwrap_err();
                assert_eq!(failure.kind(), std::io::ErrorKind::InvalidData);
                drop(failure);
                println!("INVALID_SECOND_SCUTE_REJECTED_PASS");
                continue;
            }
            let mounted = chain
                .assemble(&cache, &workspace)
                .map_err(|e| e.to_string())?;
            assert_eq!(mounted.root(), chain.root());
            let id = mounted.id();
            let node = Path::new("/dev/conglobate-chain-probe");
            rustix::fs::mknodat(
                rustix::fs::CWD,
                node,
                rustix::fs::FileType::BlockDevice,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                rustix::fs::makedev(id.major(), id.minor()),
            )?;
            if let Ok(mut writer) = fs::OpenOptions::new().write(true).open(node) {
                assert!(writer.write_all(&[0; 4096]).is_err());
            }
            let mut reader = fs::File::open(node)?;
            let mut bytes = [0; 12288];
            reader.read_exact(&mut bytes)?;
            assert_eq!(&bytes[..4096], &[66; 4096]);
            assert_eq!(&bytes[4096..8192], &[67; 4096]);
            assert_eq!(&bytes[8192..], &[0; 4096]);
            let inherited = bytes;
            let mut scratch = workspace.scratch()?;
            scratch.set_len(1024 * 1024)?;
            let sectors = NonZeroU64::new(8192).unwrap();
            let snapshot = conglobate::storage::Mapping::<conglobate::storage::Created>::create()?
                .snapshot(
                    &mounted,
                    scratch.attach().map_err(|e| e.to_string())?,
                    sectors,
                )
                .map_err(|e| e.to_string())?;
            let snapshot_node = Path::new("/dev/conglobate-chain-writable");
            let id = snapshot.id();
            rustix::fs::mknodat(
                rustix::fs::CWD,
                snapshot_node,
                rustix::fs::FileType::BlockDevice,
                rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                rustix::fs::makedev(id.major(), id.minor()),
            )?;
            let mut writer = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(snapshot_node)?;
            writer.read_exact(&mut bytes)?;
            assert_eq!(bytes, inherited);
            writer.rewind()?;
            writer.write_all(&[99; 4096])?;
            writer.seek(SeekFrom::Start(8192))?;
            writer.write_all(&[77; 4096])?;
            writer.sync_all()?;
            snapshot.usage()?;
            reader.rewind()?;
            reader.read_exact(&mut bytes)?;
            assert_eq!(bytes, inherited);
            drop(writer);
            let backing = snapshot.close()?;
            let scratch = backing.detach().map_err(|e| e.to_string())?;
            fs::remove_file(snapshot_node)?;
            // Reopen the persistent store against the same unchanged chain.
            let snapshot = conglobate::storage::Mapping::<conglobate::storage::Created>::create()?
                .snapshot(
                    &mounted,
                    scratch.attach().map_err(|e| e.to_string())?,
                    sectors,
                )
                .map_err(|e| e.to_string())?;
            let id = snapshot.id();
            rustix::fs::mknodat(
                rustix::fs::CWD,
                snapshot_node,
                rustix::fs::FileType::BlockDevice,
                rustix::fs::Mode::RUSR,
                rustix::fs::makedev(id.major(), id.minor()),
            )?;
            let mut changed = fs::File::open(snapshot_node)?;
            changed.read_exact(&mut bytes)?;
            assert_eq!(&bytes[..4096], &[99; 4096]);
            assert_eq!(&bytes[4096..8192], &[67; 4096]);
            assert_eq!(&bytes[8192..], &[77; 4096]);
            drop(changed);
            let scratch = snapshot
                .close()
                .map_err(|e| e.to_string())?
                .detach()
                .map_err(|e| e.to_string())?;
            fs::remove_file(snapshot_node)?;
            reader.rewind()?;
            reader.read_exact(&mut bytes)?;
            assert_eq!(bytes, inherited);
            assert_eq!(cache.verify_chain(&image)?.unwrap().root(), chain.root());
            println!("WRITABLE_SNAPSHOT_OVER_VERIFIED_CHAIN_PASS");
            let scute = std::sync::Arc::new(scratch.seal_snapshot(*mounted.root())?);
            let root = *scute.root();
            drop(reader);
            let mounted = mounted.extend(scute)?;
            assert_eq!(mounted.root(), &root);
            fs::remove_file(node)?;
            let id = mounted.id();
            rustix::fs::mknodat(
                rustix::fs::CWD,
                node,
                rustix::fs::FileType::BlockDevice,
                rustix::fs::Mode::RUSR,
                rustix::fs::makedev(id.major(), id.minor()),
            )?;
            let mut reader = fs::File::open(node)?;
            reader.read_exact(&mut bytes)?;
            assert_eq!(&bytes[..4096], &[99; 4096]);
            assert_eq!(&bytes[4096..8192], &[67; 4096]);
            assert_eq!(&bytes[8192..], &[77; 4096]);
            let extended = bytes;
            println!("SEALED_EXTENSION_OVER_VERIFIED_CHAIN_PASS");
            let failure = mounted.close().unwrap_err();
            assert_eq!(failure.kind(), std::io::ErrorKind::ResourceBusy);
            assert_eq!(failure.raw_os_error(), Some(16));
            reader.rewind()?;
            reader.read_exact(&mut bytes)?;
            assert_eq!(bytes, extended);
            println!("CHAIN_TERMINAL_CLOSE_RETAINS_LIVE_HOLDER_PASS");
            drop(reader);
            fs::remove_file(node)?;
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while Self::mappings()? != baseline {
                assert!(
                    std::time::Instant::now() < deadline,
                    "chain mappings leaked"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            println!("CARAPACE_CHAIN_CLEANUP_PASS");
            println!("OWNED_TWO_SCUTE_CHAIN_PASS");
            return Ok(());
        }
        println!("OWNED_TWO_SCUTE_CHAIN_PASS");
        Ok(())
    }
}
