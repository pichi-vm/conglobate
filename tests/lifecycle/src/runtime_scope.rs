// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Benign owned-file cleanup barriers, executed only in the disposable VM.
use conglobate::storage::{Runtime, Workspace};
use std::{fs, io, os::unix::fs::PermissionsExt, path::Path};

#[derive(Debug)]
pub(super) struct RuntimeScope;
impl RuntimeScope {
    fn enter_private_namespace() -> Result<(), Box<dyn std::error::Error>> {
        nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNS)?;
        rustix::mount::mount_change(
            "/",
            rustix::mount::MountPropagationFlags::PRIVATE
                | rustix::mount::MountPropagationFlags::REC,
        )?;
        Ok(())
    }

    pub(super) fn private_namespace() -> Result<(), Box<dyn std::error::Error>> {
        Self::enter_private_namespace()?;
        Self::run()?;
        println!("RUNTIME_PRIVATE_NAMESPACE_PASS");
        Ok(())
    }

    pub(super) fn changed_namespace() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = Workspace::create()?;
        let parent = Path::new("/work/runtime-changed-namespace");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let runtime = Runtime::create(&workspace, parent).map_err(|e| e.to_string())?;
        Self::enter_private_namespace()?;
        let failure = runtime.close().unwrap_err();
        assert!(failure.to_string().contains("namespace changed"));
        assert_eq!(fs::read_dir(parent)?.count(), 1);
        println!("RUNTIME_CHANGED_NAMESPACE_REJECTED_PASS");
        // No successful cleanup claim: guest shutdown disposes of both views.
        Ok(())
    }

    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = Workspace::create()?;
        let parent = Path::new("/work/runtime-scope");
        fs::create_dir(parent)?;
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
        let result = Runtime::scoped(&workspace, parent, |runtime| {
            assert!(runtime.paths()?.recovery().is_dir());
            Ok(37)
        })
        .map_err(|error| error.to_string())?;
        assert_eq!(result, 37);
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("RUNTIME_SCOPE_SUCCESS_CLOSED_PASS");

        let failure = Runtime::scoped(&workspace, parent, |_| {
            Err::<(), _>(anyhow::Error::from(io::Error::from_raw_os_error(22)))
        })
        .unwrap_err();
        assert_eq!(
            failure.downcast_ref::<io::Error>().unwrap().raw_os_error(),
            Some(22)
        );
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("RUNTIME_SCOPE_OPERATION_ERROR_CLOSED_PASS");

        for fail_operation in [false, true] {
            let parent_path = parent.join(format!("failure-{fail_operation}"));
            fs::create_dir(&parent_path)?;
            fs::set_permissions(&parent_path, fs::Permissions::from_mode(0o700))?;
            let parent = parent_path.as_path();
            let mut barrier = None;
            let failure = Runtime::scoped(&workspace, parent, |runtime| {
                let path = runtime.paths()?.recovery().join("owned-cleanup-barrier");
                fs::write(&path, b"fixture-owned ordinary file")?;
                barrier = Some(path);
                if fail_operation {
                    Err(io::Error::from_raw_os_error(22).into())
                } else {
                    Ok(37)
                }
            })
            .unwrap_err();
            if fail_operation {
                assert_eq!(
                    failure.downcast_ref::<io::Error>().unwrap().raw_os_error(),
                    Some(22)
                );
                assert!(format!("{failure:#}").contains("runtime cleanup"));
            }
            let barrier = barrier.expect("callback created barrier");
            assert_eq!(fs::read(&barrier)?, b"fixture-owned ordinary file");
            assert_eq!(fs::read_dir(parent)?.count(), 1);
            fs::remove_file(barrier)?;
            assert_eq!(fs::read_dir(parent)?.count(), 1);
        }
        println!("RUNTIME_SCOPE_CLEANUP_FAILURE_OWNERS_RETAINED_PASS");
        Ok(())
    }
}
