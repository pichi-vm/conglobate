// SPDX-License-Identifier: Apache-2.0
use std::{fs, path::Path};

#[derive(Debug)]
pub(super) struct CacheStorage;
impl CacheStorage {
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        use std::io::{Read, Write};
        let workspace = conglobate::storage::Workspace::create()?;
        let cache = conglobate::cache::Cache::open(Path::new("/cache-fixture"))?;
        for name in ["aligned", "partial", "compressed"] {
            let pin = fs::read_to_string(format!("/cache-fixture/{name}.pin"))?;
            let image = cache.resolve(pin.parse()?)?;
            let chain = cache.verify_chain(&image)?.ok_or("missing scutes")?;
            let scute = &chain.scutes()[0];
            let prepared = scute.prepare(&cache, &workspace)?;
            assert_eq!(prepared.is_cache_backed(), name == "aligned");
            assert_eq!(prepared.root(), chain.root());
            assert_eq!(prepared.data_size()?, 8192);
            assert_eq!(prepared.hash_size()?, 8192);
            let path = format!("/cache-fixture/blobs/sha256/{}", scute.digest().hex());
            let original = fs::read(&path)?;
            let verified = scute
                .prepare(&cache, &workspace)?
                .activate()
                .map_err(|e| e.to_string())?;
            let id = verified.id();
            let node = Path::new("/dev/conglobate-verity-probe");
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
            let mut holder = fs::File::open(node)?;
            let mut data = vec![0; 8192];
            holder.read_exact(&mut data)?;
            let actual_size = scute.extent().size() as usize;
            assert!(data[..actual_size].iter().all(|byte| *byte == 42));
            assert!(data[actual_size..].iter().all(|byte| *byte == 0));
            let failure = verified.close().unwrap_err();
            assert_eq!(
                failure.source_error().kind(),
                std::io::ErrorKind::ResourceBusy
            );
            let failure = failure.cleanup().unwrap_err();
            let failure = failure.cleanup().unwrap_err();
            assert_eq!(failure.diagnostics().operation().raw_os_error(), Some(16));
            assert_eq!(failure.diagnostics().cleanup().len(), 2);
            assert!(
                failure
                    .diagnostics()
                    .cleanup()
                    .iter()
                    .all(|error| error.raw_os_error() == Some(16))
            );
            assert_eq!(failure.to_string().matches("cleanup attempt").count(), 2);
            drop(holder);
            failure.cleanup().map_err(|e| e.to_string())?;
            println!("VERITY_CLEANUP_DIAGNOSTICS_RETAINED_PASS");
            fs::remove_file(node)?;
            if name == "aligned" {
                // Fresh mapping: corrupt after activation but before its first
                // read, so a previously verified page cannot mask the test.
                let verified = scute
                    .prepare(&cache, &workspace)?
                    .activate()
                    .map_err(|e| e.to_string())?;
                let id = verified.id();
                rustix::fs::mknodat(
                    rustix::fs::CWD,
                    node,
                    rustix::fs::FileType::BlockDevice,
                    rustix::fs::Mode::RUSR,
                    rustix::fs::makedev(id.major(), id.minor()),
                )?;
                let mut corruptor = fs::OpenOptions::new().write(true).open(&path)?;
                corruptor.write_all(&[1; 4096])?;
                corruptor.sync_all()?;
                drop(corruptor);
                let mut holder = fs::File::open(node)?;
                let error = holder.read_exact(&mut [0; 4096]).unwrap_err();
                assert_eq!(error.raw_os_error(), Some(5));
                drop(holder);
                verified.close().map_err(|e| e.to_string())?;
                fs::remove_file(node)?;
                fs::write(&path, &original)?;
                println!("VERITY_CACHE_CORRUPTION_READ_REJECTED_PASS");
            }
            fs::write(&path, vec![1; original.len()])?;
            assert!(scute.prepare(&cache, &workspace).is_err());
            fs::write(&path, &original)?;
            if name == "aligned" {
                fs::rename(&path, format!("{path}.original"))?;
                fs::write(&path, b"replacement")?;
                assert_eq!(prepared.data_size()?, 8192);
                assert!(scute.prepare(&cache, &workspace).is_err());
            }
        }
        println!("CACHE_BACKED_SCUTE_PREPARATION_PASS");
        println!("OWNED_VERITY_READ_AND_CLEANUP_PASS");
        Ok(())
    }
}
