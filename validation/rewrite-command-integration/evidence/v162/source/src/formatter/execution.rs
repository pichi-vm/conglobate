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
    path::Path,
    time::{Duration, Instant},
};

/// Retains both writable storage and runtime until the trusted formatter exits.
/// No image command can obtain these resources while formatting is in flight.
#[derive(Debug)]
pub struct Formatting<'origin, 'workspace> {
    process: Process<Running>,
    inputs: Box<FormatInputs<'origin, 'workspace>>,
    deadline: Instant,
}
#[derive(Debug)]
struct FormatInputs<'origin, 'workspace> {
    runtime: FormatterRuntime,
    snapshot: SnapshotMapping<'origin, 'workspace>,
    format: Ext4Format,
}
#[derive(Debug, thiserror::Error)]
#[error("formatter launch: {source}")]
pub struct FormatLaunchFailure<'origin, 'workspace> {
    source: io::Error,
    inputs: Box<FormatInputs<'origin, 'workspace>>,
}
impl<'a, 'w> FormatLaunchFailure<'a, 'w> {
    pub fn into_parts(self) -> (io::Error, FormatterRuntime, SnapshotMapping<'a, 'w>) {
        (self.source, self.inputs.runtime, self.inputs.snapshot)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("formatter wait: {source}")]
pub struct FormatWaitFailure<'origin, 'workspace> {
    source: io::Error,
    formatting: Formatting<'origin, 'workspace>,
}
impl<'a, 'w> FormatWaitFailure<'a, 'w> {
    /// Cancels the direct trusted executable, retaining all owners if it cannot
    /// be reaped. The original formatting deadline is never renewed.
    pub fn cancel(self) -> Result<(FormatterRuntime, SnapshotMapping<'a, 'w>), Self> {
        let Formatting {
            process,
            inputs,
            deadline,
        } = self.formatting;
        match process.terminate_until(deadline + Duration::from_secs(30)) {
            Ok(_) => Ok((inputs.runtime, inputs.snapshot)),
            Err(error) => {
                let (source, process) = error.into_parts();
                Err(Self {
                    source: io::Error::other(source),
                    formatting: Formatting {
                        process,
                        inputs,
                        deadline,
                    },
                })
            }
        }
    }
}
/// Child is reaped. A successful status still requires snapshot-health checks;
/// no inventory receipt or sealed layer is implied by this intermediate value.
#[derive(Debug)]
pub struct FormattedSnapshot<'origin, 'workspace> {
    inputs: Box<FormatInputs<'origin, 'workspace>>,
}
#[derive(Debug, thiserror::Error)]
#[error("formatter completion: {source}")]
pub struct FormatCompletionFailure<'origin, 'workspace> {
    source: io::Error,
    inputs: Box<FormatInputs<'origin, 'workspace>>,
}
impl<'a, 'w> FormatCompletionFailure<'a, 'w> {
    pub fn into_parts(self) -> (io::Error, FormatterRuntime, SnapshotMapping<'a, 'w>) {
        (self.source, self.inputs.runtime, self.inputs.snapshot)
    }
}
impl FormatterRuntime {
    pub fn format<'a, 'w>(
        self,
        snapshot: SnapshotMapping<'a, 'w>,
        format: Ext4Format,
        program: &HelperProgram,
        budget: SetupBudget,
    ) -> Result<Formatting<'a, 'w>, FormatLaunchFailure<'a, 'w>> {
        let inputs = Box::new(FormatInputs {
            runtime: self,
            snapshot,
            format,
        });
        let result = (|| -> io::Result<Process<Running>> {
            budget.check()?;
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
                inputs,
                deadline: budget.deadline(),
            }),
            Err(source) => Err(FormatLaunchFailure { source, inputs }),
        }
    }
}
impl<'a, 'w> Formatting<'a, 'w> {
    pub fn wait(
        self,
    ) -> Result<
        Result<FormattedSnapshot<'a, 'w>, FormatCompletionFailure<'a, 'w>>,
        FormatWaitFailure<'a, 'w>,
    > {
        let Self {
            process,
            inputs,
            deadline,
        } = self;
        let reaped = match process.wait_until(deadline) {
            Ok(reaped) => reaped,
            Err(error) => {
                let (source, process) = error.into_parts();
                return Err(FormatWaitFailure {
                    source: io::Error::other(source),
                    formatting: Self {
                        process,
                        inputs,
                        deadline,
                    },
                });
            }
        };
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
        Ok(match result {
            Ok(()) => Ok(FormattedSnapshot { inputs }),
            Err(source) => Err(FormatCompletionFailure { source, inputs }),
        })
    }
}
impl<'a, 'w> FormattedSnapshot<'a, 'w> {
    pub fn format(&self) -> &Ext4Format {
        &self.inputs.format
    }
    pub fn into_parts(self) -> (FormatterRuntime, SnapshotMapping<'a, 'w>) {
        (self.inputs.runtime, self.inputs.snapshot)
    }
}
