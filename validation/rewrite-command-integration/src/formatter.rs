// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    execution::HelperProgram,
    formatter::{CapturedFormatter, Ext4Format, FormatFailure},
    isolation::PrivateMountNamespace,
    path::ContextPath,
    step::SetupBudget,
    storage::{BootstrapVolumes, Created, Mapping, Unmounted, Volume, Workspace},
};
use std::{fs, num::NonZeroU64, path::Path};
#[derive(Debug)]
pub(super) struct FormatterProbe;
impl FormatterProbe {
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = Workspace::create()?;
        let mut scratch = workspace.scratch()?;
        scratch.set_len(32 * 1024 * 1024)?;
        let sectors =
            NonZeroU64::new(conglobate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap();
        let origin = Mapping::<Created>::create()?.zero(sectors)?;
        let snapshot = Mapping::<Created>::create()?
            .snapshot(
                &origin,
                scratch.attach().map_err(|e| e.to_string())?,
                sectors,
            )
            .map_err(|e| e.to_string())?;
        let files = fs::read_to_string("/formatter-runtime-files")?
            .lines()
            .map(|line| ContextPath::try_from(Path::new(line)))
            .collect::<Result<Vec<_>, _>>()?;
        let runtime = CapturedFormatter::capture(
            Path::new("/"),
            ContextPath::try_from(Path::new("usr/bin/mke2fs"))?,
            files,
        )?
        .with_ext4_configuration()?
        .stage(&workspace)?;
        fs::rename("/usr/bin/mke2fs", "/usr/bin/mke2fs-original")?;
        let formatted = runtime
            .format(
                snapshot,
                Ext4Format::new("16MiB".parse()?, "captured".to_owned().try_into()?),
                &HelperProgram::current()?,
                SetupBudget::begin(),
            )
            .map_err(|e| e.to_string())?
            .wait()
            .map_err(|e| e.to_string())?;
        let (runtime, snapshot) = formatted.into_parts();
        fs::rename("/usr/bin/mke2fs-original", "/usr/bin/mke2fs")?;
        // No loader/libraries in this captured root: execution must fail rather
        // than fall back to the ambient VM runtime. Reaping returns both owners.
        let executable = ContextPath::try_from(Path::new("usr/bin/mke2fs"))?;
        let incomplete =
            CapturedFormatter::capture(Path::new("/"), executable.clone(), [executable])?
                .with_ext4_configuration()?
                .stage(&workspace)?;
        let failure = incomplete
            .format(
                snapshot,
                Ext4Format::new("16MiB".parse()?, "must-fail".to_owned().try_into()?),
                &HelperProgram::current()?,
                SetupBudget::begin(),
            )
            .map_err(|e| e.to_string())?
            .wait()
            .unwrap_err();
        let (incomplete, snapshot) = match failure {
            FormatFailure::Completion(failure) => {
                let (_, runtime, snapshot) = failure.into_parts();
                (runtime, snapshot)
            }
            error => return Err(error.to_string().into()),
        };
        incomplete.close()?;
        println!("CAPTURED_FORMATTER_MISSING_LOADER_REJECTED_PASS");

        // The helper's /dev submount must not appear in the retained runtime.
        assert!(
            runtime
                .open(&ContextPath::try_from(Path::new("dev/output"))?)
                .is_err()
        );
        let mut output_scratch = workspace.scratch()?;
        output_scratch.set_len(32 * 1024 * 1024)?;
        let output = Mapping::<Created>::create()?
            .snapshot(
                &origin,
                output_scratch.attach().map_err(|e| e.to_string())?,
                sectors,
            )
            .map_err(|e| e.to_string())?;
        let formatted = runtime
            .format(
                output,
                Ext4Format::new("16MiB".parse()?, "output".to_owned().try_into()?),
                &HelperProgram::current()?,
                SetupBudget::begin(),
            )
            .map_err(|e| e.to_string())?
            .wait()
            .map_err(|e| e.to_string())?;
        let (runtime, output) = formatted.into_parts();
        runtime.close()?;
        let scope = PrivateMountNamespace::enter()?;
        let mut tool = Volume::<Unmounted>::new(snapshot, &scope);
        let mut output = Volume::<Unmounted>::new(output, &scope);
        // Exercise production pair acquisition, including rollback after the
        // first mount succeeds and the second filesystem is invalid.
        let mut blank = workspace.scratch()?;
        blank.set_len(8 * 1024 * 1024)?;
        let blank = Mapping::<Created>::create()?
            .snapshot(&origin, blank.attach().map_err(|e| e.to_string())?, sectors)
            .map_err(|e| e.to_string())?;
        let pair = BootstrapVolumes::new(tool, Volume::<Unmounted>::new(blank, &scope));
        let failure = pair
            .mount_ext4_until(Path::new("/work"), SetupBudget::begin().deadline())
            .unwrap_err();
        let original = failure.diagnostics().operation().raw_os_error();
        assert_eq!(original, Some(22));
        let paths = Self::mount_paths()?;
        assert_eq!(paths.len(), 1);
        let busy = fs::File::open(paths.first().unwrap())?;
        let failure = failure
            .rollback_until(SetupBudget::begin().deadline())
            .unwrap_err();
        assert_eq!(failure.diagnostics().operation().raw_os_error(), original);
        assert_eq!(failure.diagnostics().latest().raw_os_error(), Some(16));
        drop(busy);
        let (recovered, blank) = failure.retry().map_err(|e| e.to_string())?.into_parts();
        tool = recovered;
        drop(
            blank
                .into_snapshot()
                .close()
                .map_err(|e| e.to_string())?
                .detach()
                .map_err(|e| e.to_string())?,
        );
        assert!(Self::mount_paths()?.is_empty());
        let pair = BootstrapVolumes::new(tool, output);
        let failure = pair
            .mount_ext4_until(Path::new("/work"), std::time::Instant::now())
            .unwrap_err();
        assert_eq!(
            failure.diagnostics().operation().kind(),
            std::io::ErrorKind::TimedOut
        );
        assert!(Self::mount_paths()?.is_empty());
        let pair = failure
            .rollback_until(SetupBudget::begin().deadline())
            .map_err(|e| e.to_string())?;
        let pair = pair
            .mount_ext4_until(Path::new("/work"), SetupBudget::begin().deadline())
            .map_err(|e| e.to_string())?;
        assert_eq!(Self::mount_paths()?.len(), 2);
        (tool, output) = pair
            .unmount_until(SetupBudget::begin().deadline())
            .map_err(|e| e.to_string())?
            .into_parts();
        assert!(Self::mount_paths()?.is_empty());
        println!("BOOTSTRAP_PAIR_MOUNT_ROLLBACK_PASS");
        for busy_output in [true, false] {
            let tool_mounted = tool
                .mount_ext4(Path::new("/work"))
                .map_err(|e| e.to_string())?;
            let paths = Self::mount_paths()?;
            assert_eq!(paths.len(), 1);
            let tool_path = paths.into_iter().next().unwrap();
            let output_mounted = output
                .mount_ext4(Path::new("/work"))
                .map_err(|e| e.to_string())?;
            let paths = Self::mount_paths()?;
            assert_eq!(paths.len(), 2);
            let output_path = paths.into_iter().find(|path| *path != tool_path).unwrap();
            let busy = fs::File::open(if busy_output {
                &output_path
            } else {
                &tool_path
            })?;
            let pair = BootstrapVolumes::new(tool_mounted, output_mounted);
            let failure = pair
                .unmount_until(std::time::Instant::now() + std::time::Duration::from_secs(30))
                .unwrap_err();
            assert_eq!(failure.diagnostics().operation().raw_os_error(), Some(16));
            assert_eq!(Self::mount_paths()?.len(), if busy_output { 2 } else { 1 });
            let failure = failure.retry().unwrap_err();
            assert_eq!(failure.diagnostics().operation().raw_os_error(), Some(16));
            assert_eq!(failure.diagnostics().cleanup().len(), 1);
            drop(busy);
            (tool, output) = failure.retry().map_err(|e| e.to_string())?.into_parts();
            assert!(Self::mount_paths()?.is_empty());
        }
        let tool = tool.into_snapshot();
        let output = output.into_snapshot();
        scope.restore().map_err(|e| e.to_string())?;
        if let Some(sealed) = crate::bootstrap::BootstrapProbe::run(tool, output, &workspace)? {
            crate::sealed::SealProbe::reopen(&sealed, &workspace)?;
        }
        println!("BOOTSTRAP_PAIR_BUSY_UNMOUNT_RETRY_PASS");
        origin.close()?;
        println!("CAPTURED_FORMATTER_EXECUTION_PASS");
        Ok(())
    }
    fn mount_paths() -> std::io::Result<std::collections::BTreeSet<std::path::PathBuf>> {
        fs::read_dir("/work")?
            .filter_map(|entry| match entry {
                Ok(entry)
                    if entry
                        .file_name()
                        .as_encoded_bytes()
                        .starts_with(b"conglobate-volume-") =>
                {
                    Some(Ok(entry.path()))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }
}
