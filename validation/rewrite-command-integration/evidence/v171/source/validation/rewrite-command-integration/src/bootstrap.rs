// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    anchor::RecoveryDirectory,
    domain::RecordedDomain,
    execution::HelperProgram,
    isolation::{IdRange, Mapped, UserNamespace},
    policy::PolicyInstallation,
    process::ChildReaper,
    step::{SetupBudget, Step},
    storage::{BootstrapVolumes, SnapshotMapping, Unmounted, Volume},
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
    ) -> Result<(SnapshotMapping<'o, 'w>, SnapshotMapping<'o, 'w>), Box<dyn std::error::Error>>
    {
        let records = Path::new("/work/bootstrap-records");
        fs::create_dir(records)?;
        fs::set_permissions(records, fs::Permissions::from_mode(0o700))?;
        let lease = RecoveryDirectory::open(records)?;
        let mut tool =
            Volume::<Unmounted>::in_init_namespace(tool, &lease).map_err(|e| e.to_string())?;
        let mut output =
            Volume::<Unmounted>::in_init_namespace(output, &lease).map_err(|e| e.to_string())?;
        let parent = Path::new("/work/cgroups");
        let program = HelperProgram::current()?;
        let mut reaper = ChildReaper::install()?;
        for index in 0..2 {
            let budget = SetupBudget::begin();
            let namespace = UserNamespace::<Mapped>::create(
                program.namespace_creator(budget.cutoff()?)?,
                IdRange::new(100000, 65536)?,
                IdRange::new(100000, 65536)?,
                budget.deadline(),
            )?
            .pin(&lease)?;
            let mounted_tool = tool
                .mount_ext4(Path::new("/work"))
                .map_err(|e| e.to_string())?;
            let tool_path = Self::volume_path(mounted_tool.root_metadata()?.dev())?;
            if index == 0 {
                assert!(
                    Command::new("cp")
                        .args(["-a", "/image-root/."])
                        .arg(&tool_path)
                        .status()?
                        .success()
                );
            }
            let mounted_output = output
                .mount_ext4(Path::new("/work"))
                .map_err(|e| e.to_string())?;
            let output_path = Self::volume_path(mounted_output.root_metadata()?.dev())?;
            if index == 1 {
                assert_eq!(fs::read(output_path.join("state"))?, b"output\n");
                let metadata = fs::metadata(output_path.join("state"))?;
                assert_eq!((metadata.uid(), metadata.gid()), (123, 456));
                assert_eq!(fs::read(tool_path.join("tool-state"))?, b"tool\n");
                assert!(!output_path.join("tool-state").exists());
            }
            let policy = PolicyInstallation::new(
                namespace,
                vec![mounted_tool.pin(&lease)?, mounted_output.pin(&lease)?],
                Path::new("/work/pins"),
            )
            .install(budget.deadline())
            .map_err(|e| e.to_string())?;
            let script = if index == 0 {
                "echo tool > /tool-state; echo output > /mnt/state; chown 123:456 /mnt/state; setfattr -n security.selinux -v foreign_u:object_r:foreign_t:s0 /tool-state; setfattr -n security.selinux -v foreign_u:object_r:foreign_t:s0 /mnt/state; touch /dev/shm/outside; if setfattr -n security.selinux -v foreign_u:object_r:foreign_t:s0 /dev/shm/outside; then exit 41; fi"
            } else {
                "test $(cat /tool-state) = tool; test $(cat /mnt/state) = output; getfattr -n security.selinux /tool-state; getfattr -n security.selinux /mnt/state; echo second >> /mnt/state"
            };
            let helper = RecordedDomain::create(&lease, parent)?
                .launch(
                    program.receiver(
                        script.into(),
                        budget.cutoff()?,
                        100000,
                        100000,
                        Path::new("/work"),
                    )?,
                    budget.deadline(),
                )
                .map_err(|e| e.to_string())?;
            let step = Step::new(policy, helper, reaper)
                .with_bootstrap(BootstrapVolumes::new(mounted_tool, mounted_output));
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
        let tool = tool.into_snapshot();
        drop(reaper);
        drop(lease);
        fs::remove_dir(records)?;
        println!("BOOTSTRAP_DUAL_COMMAND_REMOUNT_PASS");
        Ok((tool, output))
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
