// SPDX-License-Identifier: Apache-2.0
//! Benign owned-file cleanup barriers, executed only in the disposable VM.
use conglobate::storage::{Runtime, RuntimeScopeFailure, Workspace};
use std::{fs, io, os::unix::fs::PermissionsExt, path::Path};

#[derive(Debug)]
pub(super) struct RuntimeScope;
impl RuntimeScope {
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = Workspace::create()?;
        let parent = Path::new("/work/runtime-scope");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let result = Runtime::scoped(&workspace, parent, |runtime| {
            assert!(runtime.paths()?.recovery().is_dir());
            Ok::<_, io::Error>(37)
        })
        .map_err(|error| error.to_string())?;
        assert_eq!(result, 37);
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("RUNTIME_SCOPE_SUCCESS_CLOSED_PASS");

        let failure = Runtime::scoped(&workspace, parent, |_| {
            Err::<(), _>(io::Error::from_raw_os_error(22))
        })
        .unwrap_err();
        let RuntimeScopeFailure::Operation(error) = failure else {
            return Err("operation error changed or cleanup failed".into());
        };
        assert_eq!(error.raw_os_error(), Some(22));
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("RUNTIME_SCOPE_OPERATION_ERROR_CLOSED_PASS");

        for fail_operation in [false, true] {
            let mut barrier = None;
            let failure = Runtime::scoped(&workspace, parent, |runtime| {
                let path = runtime.paths()?.recovery().join("owned-cleanup-barrier");
                fs::write(&path, b"fixture-owned ordinary file")?;
                barrier = Some(path);
                if fail_operation {
                    Err(io::Error::from_raw_os_error(22))
                } else {
                    Ok(37)
                }
            })
            .unwrap_err();
            let cleanup = match failure {
                RuntimeScopeFailure::Cleanup(cleanup) if !fail_operation => cleanup,
                RuntimeScopeFailure::Both { operation, cleanup } if fail_operation => {
                    assert_eq!(operation.raw_os_error(), Some(22));
                    cleanup
                }
                _ => return Err("incorrect runtime failure combination".into()),
            };
            let barrier = barrier.expect("callback created barrier");
            assert_eq!(fs::read(&barrier)?, b"fixture-owned ordinary file");
            assert_eq!(fs::read_dir(parent)?.count(), 1);
            fs::remove_file(barrier)?;
            cleanup.cleanup().map_err(|error| error.to_string())?;
            assert_eq!(fs::read_dir(parent)?.count(), 0);
        }
        println!("RUNTIME_SCOPE_CLEANUP_FAILURE_OWNERS_RETAINED_PASS");
        Ok(())
    }
}
