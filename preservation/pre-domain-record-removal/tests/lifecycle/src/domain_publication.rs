// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    anchor::RecoveryDirectory,
    domain::{CommandDomain, RecordDomainFailure},
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    time::{Duration, Instant},
};

#[derive(Debug)]
pub(super) struct DomainPublication;
impl DomainPublication {
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let directory = Path::new("/work/domain-publication-records");
        fs::create_dir(directory)?;
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        let lease = RecoveryDirectory::open(directory)?;
        let parent = Path::new("/work/cgroups");
        for mode in ["absent", "present", "malformed", "mismatch"] {
            let domain = CommandDomain::create(&lease, parent)?;
            let record = if mode == "absent" {
                None
            } else {
                Some(lease.save_domain(&domain)?)
            };
            if matches!(mode, "malformed" | "mismatch") {
                let record = record.as_ref().unwrap();
                let bytes = record.to_cbor();
                let replacement = if mode == "malformed" {
                    vec![0xff]
                } else {
                    let mut d = minicbor::Decoder::new(&bytes);
                    assert_eq!(d.array()?, Some(6));
                    let version = d.u8()?;
                    let name = d.str()?;
                    let values = [d.u64()?, d.u64()?, d.u64()?, d.u64()? ^ 1];
                    let mut e = minicbor::Encoder::new(Vec::new());
                    e.array(6)?.u8(version)?.str(name)?;
                    for value in values {
                        e.u64(value)?;
                    }
                    e.into_writer()
                };
                fs::write(
                    directory.join(format!("{}.cbor", record.name())),
                    replacement,
                )?;
            }
            // Benign fault at the publication return boundary: preserve the
            // actual created domain and whichever record state was reached.
            let failure = RecordDomainFailure::Publication {
                source: std::io::Error::from_raw_os_error(5),
                domain,
            };
            let (source, rollback) = failure.into_rollback(&lease);
            assert_eq!(source.raw_os_error(), Some(5));
            let deadline = Instant::now() + Duration::from_secs(5);
            let rollback = if matches!(mode, "malformed" | "mismatch") {
                let failure = rollback.retry_until(parent, deadline).unwrap_err();
                let (error, rollback) = failure.into_parts();
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                let record = record.as_ref().unwrap();
                assert!(parent.join(record.name()).exists());
                let retained = fs::read(directory.join(format!("{}.cbor", record.name())))?;
                assert_ne!(retained, record.to_cbor());
                if mode == "malformed" {
                    assert_eq!(retained, [0xff]);
                }
                fs::write(
                    directory.join(format!("{}.cbor", record.name())),
                    record.to_cbor(),
                )?;
                rollback
            } else {
                rollback
            };
            rollback
                .retry_until(parent, deadline)
                .map_err(|e| e.to_string())?;
            assert_eq!(fs::read_dir(directory)?.count(), 0);
            assert!(
                !fs::read_dir(parent)?.any(|entry| entry.unwrap().file_type().unwrap().is_dir())
            );
            if let Some(record) = record {
                assert!(!parent.join(record.name()).exists());
            }
            println!("DOMAIN_PUBLICATION_{}_ROLLBACK_PASS", mode.to_uppercase());
        }
        drop(lease);
        fs::remove_dir(directory)?;
        Ok(())
    }
}
