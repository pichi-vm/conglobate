// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    execution::HelperProgram,
    formatter::{CapturedFormatter, Ext4Format, FormatFailure},
    isolation::PrivateMountNamespace,
    path::ContextPath,
    step::SetupBudget,
    storage::{Created, Mapping, Unmounted, Volume, Workspace},
};
use std::{fs, num::NonZeroU64, path::Path};
#[derive(Debug)]
pub(super) struct FormatterProbe;
impl FormatterProbe {
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
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
        runtime.close()?;
        let scope = PrivateMountNamespace::enter()?;
        let volume = Volume::<Unmounted>::new(snapshot, &scope)
            .mount_ext4(Path::new("/work"))
            .map_err(|e| e.to_string())?;
        volume
            .unmount()
            .map_err(|e| e.to_string())?
            .into_snapshot()
            .close()
            .map_err(|e| e.to_string())?
            .detach()
            .map_err(|e| e.to_string())?;
        scope.restore().map_err(|e| e.to_string())?;
        origin.close()?;
        println!("CAPTURED_FORMATTER_EXECUTION_PASS");
        Ok(())
    }
}
