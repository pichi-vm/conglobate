// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Ext4Format, FormatterInput, FormatterRuntime};
use crate::{
    execution::HelperProgram,
    process::{Process, Running},
    step::SetupBudget,
    storage::{PartitionedSnapshot, SnapshotMapping},
};
use std::{
    io,
    mem::ManuallyDrop,
    path::Path,
    time::{Duration, Instant},
};

/// Retains both writable storage and runtime until the trusted formatter exits.
/// No image command can obtain these resources while formatting is in flight.
/// Abandoning an unfinished formatter retains inputs until VM disposal: the
/// process destructor's best-effort termination does not prove it was reaped.
#[derive(Debug)]
pub(crate) struct Formatting<'origin, 'workspace> {
    process: Process<Running>,
    inputs: ManuallyDrop<Box<FormatInputs<'origin, 'workspace>>>,
    deadline: Instant,
}
#[derive(Debug)]
struct FormatInputs<'origin, 'workspace> {
    runtime: FormatterRuntime,
    snapshot: PartitionedSnapshot<'origin, 'workspace>,
    format: Ext4Format,
    next: Option<std::process::Command>,
}
impl FormatInputs<'_, '_> {
    fn reject(mut self, error: anyhow::Error) -> anyhow::Error {
        self.next.take();
        let storage = self
            .snapshot
            .into_snapshot()
            .and_then(|snapshot| snapshot.close())
            .and_then(|attached| attached.detach())
            .map(|_| ());
        let runtime = self.runtime.close();
        [storage, runtime]
            .into_iter()
            .fold(error, |error, result| match result {
                Ok(()) => error,
                Err(cleanup) => error.context(format!("formatter cleanup: {cleanup}")),
            })
    }
}
/// Trusted child succeeded and was reaped; current snapshot health was checked.
/// This is not clean volume finalization, a publication receipt or a sealed layer.
#[derive(Debug)]
pub(crate) struct FormattedSnapshot<'origin, 'workspace> {
    inputs: Box<FormatInputs<'origin, 'workspace>>,
}
impl FormatterRuntime {
    pub(crate) fn format<'a, 'w>(
        self,
        snapshot: SnapshotMapping<'a, 'w>,
        format: Ext4Format,
        program: &HelperProgram,
        budget: SetupBudget,
    ) -> anyhow::Result<Formatting<'a, 'w>> {
        budget.check()?;
        let layout = format.layout()?;
        let snapshot = match PartitionedSnapshot::create(snapshot, &layout) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                return Err(match self.close() {
                    Ok(()) => error.into(),
                    Err(cleanup) => {
                        anyhow::Error::new(error).context(format!("formatter cleanup: {cleanup}"))
                    }
                });
            }
        };
        let mut inputs = Box::new(FormatInputs {
            runtime: self,
            snapshot,
            format,
            next: None,
        });
        let result = (|| -> io::Result<Process<Running>> {
            budget.check()?;
            let valid_config = inputs.runtime.captured().files().any(|(path, file)| {
                path.as_ref() == Path::new(Ext4Format::CONFIGURATION_PATH)
                    && file.bytes() == Ext4Format::configuration()
                    && !file.executable()
            });
            if !valid_config {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "formatter requires builder-owned ext4 configuration",
                ));
            }
            if let Some(format) = inputs.format.esp()? {
                let device = inputs
                    .snapshot
                    .esp_id()
                    .ok_or_else(|| io::Error::other("missing formatted ESP view"))?;
                let executable =
                    inputs.runtime.captured().fat().ok_or_else(|| {
                        io::Error::other("ESP formatting requires captured mkfs.fat")
                    })?;
                let mut command = program.formatter(
                    Path::new("/").join(executable.as_ref()),
                    format.arguments(Path::new("/dev/output")),
                    budget.cutoff()?,
                )?;
                command.stdin(FormatterInput::new(&inputs.runtime, device)?.into_stdio()?);
                inputs.next = Some(command);
            }
            let executable = Path::new("/").join(inputs.runtime.captured().executable().as_ref());
            let mut command = program.formatter(
                executable,
                inputs.format.arguments(Path::new("/dev/output")),
                budget.cutoff()?,
            )?;
            command.stdin(
                FormatterInput::new(&inputs.runtime, inputs.snapshot.root_id())?.into_stdio()?,
            );
            budget.check()?;
            command.spawn().map(Process::from)
        })();
        match result {
            Ok(process) => Ok(Formatting {
                process,
                inputs: ManuallyDrop::new(inputs),
                deadline: budget.deadline(),
            }),
            Err(source) => Err(inputs.reject(source.into())),
        }
    }
}
impl<'a, 'w> Formatting<'a, 'w> {
    pub(crate) fn wait(self) -> anyhow::Result<FormattedSnapshot<'a, 'w>> {
        let Self {
            process,
            inputs,
            deadline,
        } = self;
        let reaped = match process.wait_until(deadline) {
            Ok(reaped) => reaped,
            Err(error) => {
                let (source, process) = error.into_parts();
                let error = anyhow::Error::new(source);
                let cleanup = process.terminate_until(Instant::now() + Duration::from_secs(30));
                return Err(match cleanup {
                    Ok(_) => ManuallyDrop::into_inner(inputs).reject(error),
                    Err(failure) => {
                        let (cleanup, process) = failure.into_parts();
                        std::mem::forget(process);
                        // inputs is ManuallyDrop: unresolved child retains both roots.
                        error.context(format!("formatter cancellation: {cleanup}"))
                    }
                });
            }
        };
        // Successful wait is the proof permitting resource destruction again.
        let mut inputs = ManuallyDrop::into_inner(inputs);
        let result = (|| -> io::Result<()> {
            if Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "formatter completed after cutoff",
                ));
            }
            if !reaped.status().success() {
                return Err(io::Error::other(format!(
                    "formatter exited {}",
                    reaped.status()
                )));
            }
            inputs.snapshot.sync()?;
            Ok(())
        })();
        match result {
            Ok(()) => {
                if let Some(mut command) = inputs.next.take() {
                    let process = command.spawn().map(Process::from);
                    drop(command);
                    match process {
                        Ok(process) => Self {
                            process,
                            inputs: ManuallyDrop::new(inputs),
                            deadline,
                        }
                        .wait(),
                        Err(error) => Err(inputs.reject(error.into())),
                    }
                } else {
                    Ok(FormattedSnapshot { inputs })
                }
            }
            Err(source) => Err(inputs.reject(source.into())),
        }
    }
}
impl<'a, 'w> FormattedSnapshot<'a, 'w> {
    /// Consume successful execution; resource owners must still be closed.
    pub(crate) fn into_parts(self) -> io::Result<(FormatterRuntime, SnapshotMapping<'a, 'w>)> {
        let snapshot = self.inputs.snapshot.into_snapshot()?;
        Ok((self.inputs.runtime, snapshot))
    }
}
