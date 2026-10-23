// SPDX-License-Identifier: Apache-2.0
use conglobate::step::{CommandResult, PreparationStorage, TeardownCompletion};
use std::{fs, io::Write, path::Path};

#[derive(Debug)]
pub(super) struct Writeback;
impl Writeback {
    pub(super) fn reject<R: PreparationStorage>(
        completion: CommandResult<TeardownCompletion<R>>,
        root: &Path,
        device: &Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Benign writes target only this fixture's filesystem in a disposable VM.
        // The 8 MiB COW cannot hold 12 MiB of file data in the 16 MiB filesystem.
        let mut file = fs::File::create(root.join("exhaustion-probe"))?;
        let block = [0x5a; 256 * 1024];
        let mut write_error = None;
        for _ in 0..48 {
            if let Err(error) = file.write_all(&block) {
                write_error = Some(error);
                break;
            }
        }
        let sync_error = file.sync_all().err();
        assert!(write_error.is_some() || sync_error.is_some());
        eprintln!("fixture filesystem errors: write={write_error:?}, sync={sync_error:?}");
        assert!(
            write_error
                .iter()
                .chain(sync_error.iter())
                .any(|error| matches!(error.raw_os_error(), Some(5 | 30)))
        );
        // ext4 can report EIO directly or abort its journal and return EROFS.
        // Independently require the owned snapshot's Invalid status below.
        drop(file);
        let name = fs::read_to_string(device.join("dm/name"))?;
        let status = std::process::Command::new("/usr/bin/dmsetup")
            .args(["status", name.trim()])
            .output()?;
        assert!(status.status.success());
        let status = String::from_utf8(status.stdout)?;
        assert!(status.contains("snapshot Invalid"), "{status}");
        println!("FILESYSTEM_WRITEBACK_ERROR_OBSERVED_PASS");
        let primary = completion.outcome().to_string();
        let failure = completion.unmount().unwrap_err();
        assert_eq!(failure.outcome().to_string(), primary);
        eprintln!("initial finalization failure: {}", failure.cleanup());
        let error = failure.cleanup().diagnostics().latest();
        assert!(
            matches!(error.raw_os_error(), Some(5 | 30))
                || error.kind() == std::io::ErrorKind::InvalidData
        );
        assert!(device.exists());
        let failure = failure.retry().unwrap_err();
        assert_eq!(failure.outcome().to_string(), primary);
        assert!(!failure.outcome().success());
        eprintln!("retried finalization failure: {}", failure.cleanup());
        let error = failure.cleanup().diagnostics().latest();
        assert!(
            matches!(error.raw_os_error(), Some(5 | 30))
                || error.kind() == std::io::ErrorKind::InvalidData
        );
        assert!(device.exists());
        println!("INVALID_SNAPSHOT_FINALIZATION_RETRY_REJECTED_PASS");
        // No clean finalization or accepted storage receipt is produced. Retain
        // the failure owner until independent init disposes of the entire VM.
        std::process::exit(0);
    }
}
