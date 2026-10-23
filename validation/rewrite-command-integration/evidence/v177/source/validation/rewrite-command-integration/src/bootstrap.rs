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
    ) -> Result<conglobate::storage::SealedScute<'w>, Box<dyn std::error::Error>> {
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
        let (tool_store, tool_pin) =
            crate::sealed::FixtureCache::import(Path::new("/work/tool-cache"), &tool)?;
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

        for failure_mode in ["domain", "policy", "launch"] {
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
            reaper = ChildReaper::install()?;
            assert_eq!(fs::read_dir(records)?.count(), 0);
            assert_eq!(fs::read_dir("/work/pins")?.count(), 0);
            assert!(!fs::read_dir("/work")?.any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .as_encoded_bytes()
                    .starts_with(b"conglobate-volume-")
            }));
        }
        println!("BOOTSTRAP_PREPARATION_ROLLBACK_PASS");
        let sandbox = CommandSandbox::new(
            program,
            SandboxDirectories::new(&lease, Path::new("/work"), Path::new("/work/pins"), parent),
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
        );
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
            step.send_bootstrap_until(budget.deadline())?;
            let prepared = match budget.await_prepared(step) {
                Ok(prepared) => prepared,
                Err(failure) => {
                    let (error, step) = failure.into_parts();
                    eprintln!("dual bootstrap readiness: {error}");
                    let teardown = step.teardown(parent).map_err(|e| e.to_string())?;
                    let _ = teardown.unmount().map_err(|e| e.to_string())?;
                    return Err(error.into());
                }
            };
            let command = prepared.start("5s".parse()?).map_err(|e| e.to_string())?;
            let (outcome, teardown) = command.observe().teardown(parent).into_parts();
            let finalized = teardown
                .map_err(|e| e.to_string())?
                .unmount()
                .map_err(|e| e.to_string())?;
            assert!(outcome.success(), "{outcome:?}");
            let (_, reaped, volumes) = finalized.into_parts();
            (tool, output) = volumes.into_parts();
            reaper = reaped.into_reaper();
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
        Ok(sealed)
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
