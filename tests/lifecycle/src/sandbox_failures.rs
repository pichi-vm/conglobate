// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    execution::HelperProgram,
    isolation::IdRange,
    process::ChildReaper,
    step::{CommandSandbox, SetupBudget},
    storage::{Created, Mapping, Runtime, Unmounted, Volume, Workspace},
};
use std::{fs, num::NonZeroU64, os::unix::fs::PermissionsExt, path::Path};

#[derive(Debug, Clone, Copy)]
pub(super) enum FailurePoint {
    Namespace,
    Mount,
}
#[derive(Debug)]
pub(super) struct SandboxFailures;
impl SandboxFailures {
    pub(super) fn run(point: FailurePoint) -> Result<(), Box<dyn std::error::Error>> {
        let workspace = Workspace::create()?;
        let mut scratch = workspace.scratch()?;
        scratch.set_len(32 * 1024 * 1024)?;
        let sectors = NonZeroU64::new(32768).unwrap();
        let origin = Mapping::<Created>::create()?.zero(sectors)?;
        let snapshot = Mapping::<Created>::create()?
            .snapshot(
                &origin,
                scratch.attach().map_err(|e| e.to_string())?,
                sectors,
            )
            .map_err(|e| e.to_string())?;
        let device = snapshot.id();
        let runtime_parent = Path::new("/work/runtime-parent");
        fs::create_dir(runtime_parent)?;
        fs::set_permissions(runtime_parent, fs::Permissions::from_mode(0o700))?;
        let runtime = Runtime::create(&workspace, runtime_parent).map_err(|e| e.to_string())?;
        let paths = runtime.paths()?;
        let volume = Volume::<Unmounted>::in_init_namespace(snapshot, runtime.recovery())
            .map_err(|e| e.to_string())?;
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
        );
        let reaper = ChildReaper::install()?;
        let limit = Path::new("/proc/sys/user/max_user_namespaces");
        let original = fs::read_to_string(limit)?;
        if matches!(point, FailurePoint::Namespace) {
            // This sysctl exists only in the disposable guest. Restore before
            // assertions or rollback; no command can run in this fixture.
            fs::write(limit, "0")?;
        }
        let result = sandbox.prepare(volume, reaper, &SetupBudget::begin());
        if matches!(point, FailurePoint::Namespace) {
            fs::write(limit, original)?;
        }
        let failure = result.unwrap_err();
        let kind = failure.diagnostics().operation().kind();
        let errno = failure.diagnostics().operation().raw_os_error();
        assert_ne!(kind, std::io::ErrorKind::TimedOut);
        if matches!(point, FailurePoint::Mount) {
            // The snapshot intentionally contains no filesystem superblock.
            assert_eq!(errno, Some(22));
        }
        let (diagnostics, returned) = failure.rollback().map_err(|e| e.to_string())?.into_parts();
        assert_eq!(diagnostics.operation().kind(), kind);
        assert_eq!(diagnostics.operation().raw_os_error(), errno);
        assert!(diagnostics.cleanup().is_empty());
        assert_eq!(fs::read_dir(paths.recovery())?.count(), 0);
        assert_eq!(fs::read_dir(paths.volumes())?.count(), 0);
        for entry in fs::read_dir(paths.domains())? {
            assert!(!entry?.file_type()?.is_dir());
        }
        assert!(
            fs::read_to_string("/proc/thread-self/children")?
                .trim()
                .is_empty()
        );
        let snapshot = returned.into_snapshot();
        snapshot.usage()?;
        snapshot
            .close()
            .map_err(|e| e.to_string())?
            .detach()
            .map_err(|e| e.to_string())?;
        assert!(
            !Path::new(&format!(
                "/sys/dev/block/{}:{}",
                device.major(),
                device.minor()
            ))
            .exists()
        );
        origin.close()?;
        drop(sandbox);
        drop(paths);
        runtime.close().map_err(|e| e.to_string())?;
        assert_eq!(fs::read_dir(runtime_parent)?.count(), 0);
        fs::remove_dir(runtime_parent)?;
        println!(
            "{}",
            match point {
                FailurePoint::Namespace => "SANDBOX_NAMESPACE_FAILURE_ROLLBACK_PASS",
                FailurePoint::Mount => "SANDBOX_MOUNT_FAILURE_ROLLBACK_PASS",
            }
        );
        Ok(())
    }
}
