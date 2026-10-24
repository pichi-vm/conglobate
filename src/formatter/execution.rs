// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Ext4Format, FormatterInput, FormatterRuntime};
use crate::{
    execution::HelperProgram,
    process::{Process, Running},
    step::SetupBudget,
    storage::SnapshotMapping,
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
    snapshot: SnapshotMapping<'origin, 'workspace>,
    format: Ext4Format,
}
impl FormatInputs<'_, '_> {
    fn reject(self, error: anyhow::Error) -> anyhow::Error {
        let storage = self
            .snapshot
            .close()
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
/// This is not clean volume finalization, an inventory receipt or a sealed layer.
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
        let inputs = Box::new(FormatInputs {
            runtime: self,
            snapshot,
            format,
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
            let executable = Path::new("/").join(inputs.runtime.captured().executable().as_ref());
            let mut command = program.formatter(
                executable,
                inputs.format.arguments(Path::new("/dev/output")),
                budget.cutoff()?,
            )?;
            command
                .stdin(FormatterInput::new(&inputs.runtime, inputs.snapshot.id())?.into_stdio()?);
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
        let inputs = ManuallyDrop::into_inner(inputs);
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
            inputs.snapshot.usage()?;
            Ok(())
        })();
        match result {
            Ok(()) => Ok(FormattedSnapshot { inputs }),
            Err(source) => Err(inputs.reject(source.into())),
        }
    }
}
/// Evidence of one successful, reaped formatter execution and healthy snapshot.
/// It does not certify later commands, sealing, or output publication. There is
/// no constructor from a recipe, captured runtime, or decoded inventory.
///
#[derive(Debug)]
pub(crate) struct FormatReceipt {
    format: Ext4Format,
    executable: crate::path::ContextPath,
    runtime: Vec<(crate::path::ContextPath, [u8; 32])>,
}
impl FormatReceipt {
    pub(crate) fn format(&self) -> &Ext4Format {
        &self.format
    }
    pub(crate) fn executable(&self) -> &crate::path::ContextPath {
        &self.executable
    }
    pub(crate) fn runtime(&self) -> &[(crate::path::ContextPath, [u8; 32])] {
        &self.runtime
    }
}
impl<'a, 'w> FormattedSnapshot<'a, 'w> {
    /// Consume successful execution while retaining its exact input identities.
    /// Resource owners remain separate so the coordinator must still close them.
    pub(crate) fn into_recorded_parts(
        self,
    ) -> (FormatterRuntime, SnapshotMapping<'a, 'w>, FormatReceipt) {
        let captured = self.inputs.runtime.captured();
        let receipt = FormatReceipt {
            executable: captured.executable().clone(),
            runtime: captured
                .files()
                .map(|(path, file)| (path.clone(), *file.digest()))
                .collect(),
            format: self.inputs.format,
        };
        (self.inputs.runtime, self.inputs.snapshot, receipt)
    }
    #[cfg(feature = "test-fixtures")]
    pub(crate) fn into_parts(self) -> (FormatterRuntime, SnapshotMapping<'a, 'w>) {
        (self.inputs.runtime, self.inputs.snapshot)
    }
}
