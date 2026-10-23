// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    execution::HelperProgram,
    formatter::{CapturedFormatter, Ext4Format, FormatFailure},
    path::ContextPath,
    process::WaitError,
    step::SetupBudget,
    storage::{Created, Mapping, Workspace},
};
use std::{fs, num::NonZeroU64, path::Path};

#[derive(Debug)]
pub(super) struct FormatterFailures;
impl FormatterFailures {
    /// A trusted fixture formatter that blocks without touching the output.
    /// Cancellation must reap this process before returning its storage owner.
    pub(super) fn stall() -> ! {
        println!("FORMATTER_STALL_ENTERED");
        loop {
            std::thread::park();
        }
    }
    pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = Workspace::create()?;
        let sectors =
            NonZeroU64::new(conglobate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap();
        let origin = Mapping::<Created>::create()?.zero(sectors)?;
        for exhausted in [false, true] {
            let mut scratch = workspace.scratch()?;
            scratch.set_len(if exhausted { 64 * 1024 } else { 1024 * 1024 })?;
            let snapshot = Mapping::<Created>::create()?
                .snapshot(
                    &origin,
                    scratch.attach().map_err(|e| e.to_string())?,
                    sectors,
                )
                .map_err(|e| e.to_string())?;
            let (executable, manifest) = if exhausted {
                ("usr/bin/mke2fs", "/formatter-runtime-files")
            } else {
                ("usr/bin/formatter-stall", "/formatter-stall-files")
            };
            let files = fs::read_to_string(manifest)?
                .lines()
                .map(|line| ContextPath::try_from(Path::new(line)))
                .collect::<Result<Vec<_>, _>>()?;
            let runtime = CapturedFormatter::capture(
                Path::new("/"),
                ContextPath::try_from(Path::new(executable))?,
                files,
            )?
            .with_ext4_configuration()?
            .stage(&workspace)?;
            let budget = SetupBudget::begin();
            let deadline = budget.deadline();
            let device = snapshot.id();
            let formatting = runtime
                .format(
                    snapshot,
                    Ext4Format::new("16MiB".parse()?, "failure-probe".to_owned().try_into()?),
                    &HelperProgram::current()?,
                    budget,
                )
                .map_err(|e| e.to_string())?;
            if !exhausted && std::env::var("FORMATTER_DROP").as_deref() == Ok("running") {
                drop(formatting);
                assert!(
                    Path::new(&format!(
                        "/sys/dev/block/{}:{}",
                        device.major(),
                        device.minor()
                    ))
                    .exists()
                );
                println!("FORMATTER_DROP_RUNNING_RETAINS_STORAGE_PASS");
                // Terminal VM-disposal test: do not let unrelated outer RAII
                // cleanup obscure whether the formatter retained its own inputs.
                std::process::exit(0);
            }
            let failure = formatting.wait().unwrap_err();
            let (runtime, snapshot) = match (exhausted, failure) {
                (false, FormatFailure::Wait(failure)) => {
                    assert!(std::time::Instant::now() >= deadline);
                    assert!(matches!(
                        failure
                            .diagnostics()
                            .operation()
                            .get_ref()
                            .and_then(|error| error.downcast_ref::<WaitError>()),
                        Some(WaitError::Deadline)
                    ));
                    if std::env::var("FORMATTER_DROP").as_deref() == Ok("wait") {
                        drop(failure);
                        assert!(
                            Path::new(&format!(
                                "/sys/dev/block/{}:{}",
                                device.major(),
                                device.minor()
                            ))
                            .exists()
                        );
                        println!("FORMATTER_DROP_WAIT_RETAINS_STORAGE_PASS");
                        std::process::exit(0);
                    }
                    let (diagnostics, runtime, snapshot) =
                        failure.cancel().map_err(|e| e.to_string())?;
                    assert!(diagnostics.cleanup().is_empty());
                    assert!(
                        fs::read_to_string("/proc/thread-self/children")?
                            .trim()
                            .is_empty()
                    );
                    snapshot.usage()?;
                    println!("FORMATTER_DEADLINE_CANCEL_REAP_PASS");
                    (runtime, snapshot)
                }
                (true, FormatFailure::Completion(failure)) => {
                    let (_, runtime, snapshot) = failure.into_parts();
                    assert_eq!(
                        snapshot.usage().unwrap_err().kind(),
                        std::io::ErrorKind::InvalidData
                    );
                    assert!(
                        fs::read_to_string("/proc/thread-self/children")?
                            .trim()
                            .is_empty()
                    );
                    println!("FORMATTER_EXHAUSTED_SNAPSHOT_REJECTED_PASS");
                    (runtime, snapshot)
                }
                (_, failure) => {
                    return Err(format!("unexpected formatter failure: {failure}").into());
                }
            };
            runtime.close()?;
            snapshot
                .close()
                .map_err(|e| e.to_string())?
                .detach()
                .map_err(|e| e.to_string())?;
        }
        origin.close()?;
        println!("FORMATTER_FAILURE_RESOURCES_RELEASED_PASS");
        Ok(())
    }
}
