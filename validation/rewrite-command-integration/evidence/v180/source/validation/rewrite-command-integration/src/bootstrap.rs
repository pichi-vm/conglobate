// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    anchor::RecoveryDirectory,
    execution::HelperProgram,
    isolation::IdRange,
    process::ChildReaper,
    step::{CommandSandbox, SandboxDirectories, SetupBudget},
    storage::{BootstrapVolumes, Created, Mapping, SnapshotMapping, Unmounted, Volume, Workspace},
};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::Path,
    process::Command,
};
#[derive(Debug)]
pub(super) struct BootstrapProbe;
impl BootstrapProbe {
    pub(super) fn run<'o, 'w>(
        tool: SnapshotMapping<'o, 'w>,
        output: SnapshotMapping<'o, 'w>,
        workspace: &'w Workspace,
    ) -> Result<Option<conglobate::storage::SealedScute<'w>>, Box<dyn std::error::Error>> {
        let failure_mode = std::env::var("BOOTSTRAP_FAILURE").ok();
        let command_mode = std::env::var("BOOTSTRAP_COMMAND_FAILURE").ok();
        let records = Path::new("/work/bootstrap-records");
        fs::create_dir(records)?;
        fs::set_permissions(records, fs::Permissions::from_mode(0o700))?;
        let lease = RecoveryDirectory::open(records)?;
        let tool =
            Volume::<Unmounted>::in_init_namespace(tool, &lease).map_err(|e| e.to_string())?;
        let mut output =
            Volume::<Unmounted>::in_init_namespace(output, &lease).map_err(|e| e.to_string())?;
        let parent = Path::new("/work/cgroups");
        let program = HelperProgram::current()?;
        let mut reaper = ChildReaper::install()?;
        // Trusted fixture setup makes a tool artifact; bootstrap execution then
        // consumes only the verified cache chain and a disposable overlay.
        let mounted_tool = tool
            .mount_ext4(Path::new("/work"))
            .map_err(|e| e.to_string())?;
        let tool_path = Self::volume_path(mounted_tool.root_metadata()?.dev())?;
        assert!(
            Command::new("cp")
                .args(["-a", "/image-root/."])
                .arg(&tool_path)
                .status()?
                .success()
        );
        let tool = mounted_tool
            .unmount()
            .map_err(|e| e.to_string())?
            .into_snapshot()
            .close()
            .map_err(|e| e.to_string())?
            .detach()
            .map_err(|e| e.to_string())?
            .seal_snapshot([0; 32])
            .map_err(|e| e.to_string())?;
        let (tool_store, tool_pin) = crate::sealed::FixtureCache::import(
            &Path::new("/work").join(format!(
                "tool-cache-{}",
                failure_mode
                    .as_deref()
                    .or(command_mode.as_deref())
                    .unwrap_or("success")
            )),
            &tool,
        )?;
        let tool_cache = tool_store.open()?;
        let tool_image = tool_cache.resolve(tool_pin)?;
        let verified_tool = tool_cache
            .verify_chain(&tool_image)?
            .ok_or("missing tool chain")?;
        assert_eq!(verified_tool.root(), tool.root());
        let active_tool = verified_tool
            .assemble(&tool_cache, workspace)
            .map_err(|e| e.to_string())?;
        let mut tool_cow = workspace.scratch()?;
        tool_cow.set_len(32 * 1024 * 1024)?;
        let sectors = std::num::NonZeroU64::new(
            conglobate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512,
        )
        .unwrap();
        let tool_snapshot = Mapping::<Created>::create()?
            .snapshot(
                &active_tool,
                tool_cow.attach().map_err(|e| e.to_string())?,
                sectors,
            )
            .map_err(|e| e.to_string())?;
        let mut tool = Volume::<Unmounted>::in_init_namespace(tool_snapshot, &lease)
            .map_err(|e| e.to_string())?;

        if let Some(failure_mode) = failure_mode.as_deref() {
            let budget = SetupBudget::begin();
            let sandbox = CommandSandbox::new(
                HelperProgram::current()?,
                SandboxDirectories::new(
                    &lease,
                    Path::new("/work"),
                    if failure_mode == "policy" {
                        Path::new("/missing-policy-parent")
                    } else {
                        Path::new("/work/pins")
                    },
                    if failure_mode == "domain" {
                        Path::new("/missing-domain-parent")
                    } else {
                        parent
                    },
                ),
                IdRange::new(100000, 65536)?,
                IdRange::new(100000, 65536)?,
            );
            let preparation =
                sandbox.prepare_bootstrap(BootstrapVolumes::new(tool, output), reaper, &budget);
            let failure = if failure_mode == "launch" {
                preparation
                    .map_err(|e| e.to_string())?
                    .launch(
                        Command::new("/missing-bootstrap-helper"),
                        budget.deadline(),
                        parent,
                    )
                    .unwrap_err()
            } else {
                preparation.unwrap_err()
            };
            assert_eq!(failure.diagnostics().operation().raw_os_error(), Some(2));
            let (diagnostics, volumes) =
                failure.rollback().map_err(|e| e.to_string())?.into_parts();
            assert_eq!(diagnostics.operation().raw_os_error(), Some(2));
            (tool, output) = volumes.into_parts();
            tool.into_snapshot()
                .close()
                .map_err(|e| e.to_string())?
                .detach()
                .map_err(|e| e.to_string())?;
            output
                .into_snapshot()
                .close()
                .map_err(|e| e.to_string())?
                .detach()
                .map_err(|e| e.to_string())?;
            active_tool.close().map_err(|e| e.to_string())?;
            assert_eq!(fs::read_dir(records)?.count(), 0);
            assert_eq!(fs::read_dir("/work/pins")?.count(), 0);
            assert!(!fs::read_dir("/work")?.any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b"conglobate-volume-")
            }));
            drop(lease);
            fs::remove_dir(records)?;
            println!(
                "BOOTSTRAP_PREPARATION_{}_ROLLBACK_PASS",
                failure_mode.to_uppercase()
            );
            return Ok(None);
        }
        let sandbox = CommandSandbox::new(
            program,
            SandboxDirectories::new(&lease, Path::new("/work"), Path::new("/work/pins"), parent),
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
        );
        if let Some(mode) = command_mode.as_deref() {
            let budget = SetupBudget::begin();
            let script = match mode {
                "timeout" => "echo partial > /mnt/state; while :; do :; done",
                "success-busy" => "echo output > /mnt/state; echo tool > /tool-state",
                _ => "echo partial > /mnt/state; echo partial > /tool-state; exit 17",
            };
            let receiver = sandbox.receiver(script.into(), &budget)?;
            let preparation = sandbox
                .prepare_bootstrap(BootstrapVolumes::new(tool, output), reaper, &budget)
                .map_err(|e| e.to_string())?;
            let holder = if mode.ends_with("busy") {
                // Fixture-only open handle deterministically makes output unmount busy.
                let point = fs::read_dir("/work")?
                    .find_map(|entry| {
                        let entry = entry.ok()?;
                        (entry
                            .file_name()
                            .as_encoded_bytes()
                            .starts_with(b"conglobate-volume-")
                            && !entry.path().join("bin").exists())
                        .then(|| entry.path())
                    })
                    .ok_or("missing bootstrap output mount")?;
                Some(fs::File::open(point)?)
            } else {
                None
            };
            let step = preparation
                .launch(receiver, budget.deadline(), parent)
                .map_err(|e| e.to_string())?;
            let failure = step
                .execute(
                    budget,
                    if mode == "timeout" { "100ms" } else { "5s" }.parse()?,
                    parent,
                )
                .unwrap_err();
            match (mode, failure.outcome()) {
                (
                    "timeout",
                    conglobate::step::ExecutionOutcome::Command(
                        conglobate::step::CommandOutcome::ObservationFailed(error),
                    ),
                ) => {
                    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
                }
                (
                    "success-busy",
                    conglobate::step::ExecutionOutcome::Command(
                        conglobate::step::CommandOutcome::Exited(status),
                    ),
                ) => assert!(status.success()),
                (
                    _,
                    conglobate::step::ExecutionOutcome::Command(
                        conglobate::step::CommandOutcome::Exited(status),
                    ),
                ) => assert_eq!(status.code(), Some(17)),
                (_, outcome) => panic!("unexpected bootstrap outcome: {outcome:?}"),
            }
            let before = format!("{:?}", failure.outcome());
            let failure = if holder.is_some() {
                assert_eq!(
                    failure.cleanup_error().unwrap().latest().raw_os_error(),
                    Some(16)
                );
                let failure = failure.into_rejected_storage().unwrap_err();
                let failure = failure.retry_cleanup().unwrap_err();
                assert_eq!(format!("{:?}", failure.outcome()), before);
                drop(holder);
                failure
            } else {
                assert!(failure.cleanup_error().is_none());
                failure
            };
            let volumes = if mode == "success-busy" {
                failure
                    .retry_cleanup()
                    .map_err(|e| e.to_string())?
                    .into_storage()
            } else {
                let failure = failure.retry_cleanup().unwrap_err();
                assert_eq!(format!("{:?}", failure.outcome()), before);
                assert!(failure.cleanup_error().is_none());
                let (_, volumes) = failure.into_rejected_storage().map_err(|e| e.to_string())?;
                volumes
            };
            let (tool, output) = volumes.into_parts();
            tool.into_snapshot()
                .close()
                .map_err(|e| e.to_string())?
                .detach()
                .map_err(|e| e.to_string())?;
            output
                .into_snapshot()
                .close()
                .map_err(|e| e.to_string())?
                .detach()
                .map_err(|e| e.to_string())?;
            active_tool.close().map_err(|e| e.to_string())?;
            assert_eq!(fs::read_dir(records)?.count(), 0);
            assert_eq!(fs::read_dir("/work/pins")?.count(), 0);
            assert!(!fs::read_dir("/work")?.any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b"conglobate-volume-")
            }));
            drop(lease);
            fs::remove_dir(records)?;
            println!("BOOTSTRAP_EXECUTION_{}_PASS", mode.to_uppercase());
            return Ok(None);
        }
        for index in 0..2 {
            let budget = SetupBudget::begin();
            let script = if index == 0 {
                "echo tool > /tool-state; echo output > /mnt/state; chown 123:456 /mnt/state; setfattr -n security.selinux -v foreign_u:object_r:foreign_t:s0 /tool-state; setfattr -n security.selinux -v foreign_u:object_r:foreign_t:s0 /mnt/state; touch /dev/shm/outside; if setfattr -n security.selinux -v foreign_u:object_r:foreign_t:s0 /dev/shm/outside; then exit 41; fi"
            } else {
                "test $(cat /tool-state) = tool; test $(cat /mnt/state) = output; getfattr -n security.selinux /tool-state; getfattr -n security.selinux /mnt/state; echo second >> /mnt/state"
            };
            let receiver = sandbox.receiver(script.into(), &budget)?;
            let preparation = sandbox
                .prepare_bootstrap(BootstrapVolumes::new(tool, output), reaper, &budget)
                .map_err(|e| e.to_string())?;
            let step = preparation
                .launch(receiver, budget.deadline(), parent)
                .map_err(|e| e.to_string())?;
            let executed = step
                .execute(budget, "5s".parse()?, parent)
                .map_err(|e| e.to_string())?;
            let (volumes, next_reaper) = executed.into_parts();
            (tool, output) = volumes.into_parts();
            reaper = next_reaper;
            assert_eq!(fs::read_dir(records)?.count(), 0);
            assert_eq!(fs::read_dir("/work/pins")?.count(), 0);
        }
        let output_mounted = output
            .mount_ext4(Path::new("/work"))
            .map_err(|e| e.to_string())?;
        let path = Self::volume_path(output_mounted.root_metadata()?.dev())?;
        assert_eq!(fs::read(path.join("state"))?, b"output\nsecond\n");
        let output = output_mounted
            .unmount()
            .map_err(|e| e.to_string())?
            .into_snapshot();
        let output = output
            .close()
            .map_err(|e| e.to_string())?
            .detach()
            .map_err(|e| e.to_string())?;
        let capacity = output.metadata()?.len();
        let sealed = output.seal_snapshot([0; 32]).map_err(|e| e.to_string())?;
        assert!(sealed.descriptor().size < capacity);
        tool.into_snapshot()
            .close()
            .map_err(|e| e.to_string())?
            .detach()
            .map_err(|e| e.to_string())?;
        active_tool.close().map_err(|e| e.to_string())?;
        assert_eq!(
            tool_cache.verify_chain(&tool_image)?.unwrap().root(),
            verified_tool.root()
        );
        println!("BOOTSTRAP_VERIFIED_TOOL_CHAIN_PASS");
        drop(lease);
        fs::remove_dir(records)?;
        println!("BOOTSTRAP_DUAL_COMMAND_REMOUNT_PASS");
        Ok(Some(sealed))
    }
    fn volume_path(device: u64) -> std::io::Result<std::path::PathBuf> {
        for entry in fs::read_dir("/work")? {
            let entry = entry?;
            if entry
                .file_name()
                .as_encoded_bytes()
                .starts_with(b"conglobate-volume-")
                && entry.metadata()?.dev() == device
            {
                return Ok(entry.path());
            }
        }
        Err(std::io::Error::other("missing fixture volume"))
    }
}
