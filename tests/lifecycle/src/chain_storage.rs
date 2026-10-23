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

    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let baseline = Self::mappings()?;
        let workspace = conglobate::storage::Workspace::create()?;
        let cache = conglobate::cache::Cache::open(Path::new("/cache-fixture"))?;
        for name in ["chain", "invalid-header"] {
            let pin = fs::read_to_string(format!("/cache-fixture/{name}.pin"))?;
            let image = cache.resolve(pin.parse()?)?;
            let chain = cache.verify_chain(&image)?.ok_or("missing chain")?;
            if name == "invalid-header" {
                let failure = chain.assemble(&cache, &workspace).unwrap_err();
                assert_eq!(
                    failure.source_error().kind(),
                    std::io::ErrorKind::InvalidData
                );
                failure.cleanup().map_err(|e| e.to_string())?;
                assert_eq!(Self::mappings()?, baseline);
                println!("INVALID_SECOND_SCUTE_CLEANUP_PASS");
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
            let failure = snapshot.close().unwrap_err();
            assert_eq!(
                failure.source_error().kind(),
                std::io::ErrorKind::ResourceBusy
            );
            drop(writer);
            let backing = failure.cleanup().map_err(|e| e.to_string())?;
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
            snapshot
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
            let failure = mounted.close().unwrap_err();
            assert_eq!(
                failure.source_error().kind(),
                std::io::ErrorKind::ResourceBusy
            );
            let failure = failure.cleanup().unwrap_err();
            assert_eq!(failure.diagnostics().operation().raw_os_error(), Some(16));
            assert_eq!(failure.diagnostics().cleanup().len(), 1);
            assert_eq!(failure.diagnostics().cleanup()[0].raw_os_error(), Some(16));
            assert_eq!(failure.to_string().matches("cleanup attempt").count(), 1);
            drop(reader);
            failure.cleanup().map_err(|e| e.to_string())?;
            println!("CHAIN_CLEANUP_DIAGNOSTICS_RETAINED_PASS");
            fs::remove_file(node)?;
            assert_eq!(Self::mappings()?, baseline);
        }
        println!("OWNED_TWO_SCUTE_CHAIN_PASS");
        Ok(())
    }
}
