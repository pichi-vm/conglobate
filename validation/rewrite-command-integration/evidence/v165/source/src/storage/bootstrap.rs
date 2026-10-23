// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{FailureDiagnostics, Mounted, Unmounted, Volume, VolumeState, VolumeUnmountFailure};
use std::io;

/// The disposable tool root and persistent output have one lifetime owner.
/// State denotes both volumes; partial finalization exists only in its owning
/// failure, never as an independently extractable output.
#[derive(Debug)]
pub struct BootstrapVolumes<'ns, 'origin, 'workspace, S: VolumeState> {
    tool: Volume<'ns, 'origin, 'workspace, S>,
    output: Volume<'ns, 'origin, 'workspace, S>,
}
impl<'n, 'o, 'w, S: VolumeState> BootstrapVolumes<'n, 'o, 'w, S> {
    pub fn new(tool: Volume<'n, 'o, 'w, S>, output: Volume<'n, 'o, 'w, S>) -> Self {
        Self { tool, output }
    }
}
impl<'n, 'o, 'w> BootstrapVolumes<'n, 'o, 'w, Unmounted> {
    pub fn into_parts(self) -> (Volume<'n, 'o, 'w, Unmounted>, Volume<'n, 'o, 'w, Unmounted>) {
        (self.tool, self.output)
    }
    pub(crate) fn validate_snapshots(&self) -> io::Result<()> {
        self.tool.validate_snapshot()?;
        self.output.validate_snapshot()
    }
}
#[derive(Debug)]
enum Pending<'n, 'o, 'w> {
    OutputReady(BootstrapVolumes<'n, 'o, 'w, Mounted>),
    OutputRetry {
        tool: Volume<'n, 'o, 'w, Mounted>,
        output: VolumeUnmountFailure<'n, 'o, 'w>,
    },
    ToolReady {
        tool: Volume<'n, 'o, 'w, Mounted>,
        output: Volume<'n, 'o, 'w, Unmounted>,
    },
    ToolRetry {
        tool: VolumeUnmountFailure<'n, 'o, 'w>,
        output: Volume<'n, 'o, 'w, Unmounted>,
    },
    Complete(BootstrapVolumes<'n, 'o, 'w, Unmounted>),
}
#[derive(Debug)]
struct Unmounting<'n, 'o, 'w> {
    pending: Pending<'n, 'o, 'w>,
    deadline: Option<std::time::Instant>,
}
/// Neither volume can escape while either remains mounted or needs cleanup.
///
/// ```compile_fail
/// use conglobate::storage::BootstrapUnmountFailure;
/// fn cannot_extract_partial_pair(failure: BootstrapUnmountFailure<'_, '_, '_>) {
///     failure.into_parts();
/// }
/// ```
#[derive(Debug, thiserror::Error)]
#[error("bootstrap volumes unmount: {diagnostics}")]
pub struct BootstrapUnmountFailure<'n, 'o, 'w> {
    diagnostics: FailureDiagnostics,
    unmounting: Unmounting<'n, 'o, 'w>,
}
impl<'n, 'o, 'w> BootstrapVolumes<'n, 'o, 'w, Mounted> {
    pub fn unmount(
        self,
    ) -> Result<BootstrapVolumes<'n, 'o, 'w, Unmounted>, Box<BootstrapUnmountFailure<'n, 'o, 'w>>>
    {
        Unmounting {
            pending: Pending::OutputReady(self),
            deadline: None,
        }
        .run()
    }
    /// Keep the supplied cutoff through both unmounts and all retries. Used by
    /// step finalization with its existing teardown deadline, never a new budget.
    pub fn unmount_until(
        self,
        deadline: std::time::Instant,
    ) -> Result<BootstrapVolumes<'n, 'o, 'w, Unmounted>, Box<BootstrapUnmountFailure<'n, 'o, 'w>>>
    {
        Unmounting {
            pending: Pending::OutputReady(self),
            deadline: Some(deadline),
        }
        .run()
    }
}
impl<'n, 'o, 'w> BootstrapUnmountFailure<'n, 'o, 'w> {
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn retry(self) -> Result<BootstrapVolumes<'n, 'o, 'w, Unmounted>, Box<Self>> {
        self.unmounting.run().map_err(|failure| {
            Box::new(Self {
                diagnostics: self.diagnostics.followed_by(failure.diagnostics),
                unmounting: failure.unmounting,
            })
        })
    }
}
impl<'n, 'o, 'w> Unmounting<'n, 'o, 'w> {
    fn run(
        mut self,
    ) -> Result<BootstrapVolumes<'n, 'o, 'w, Unmounted>, Box<BootstrapUnmountFailure<'n, 'o, 'w>>>
    {
        loop {
            if self
                .deadline
                .is_some_and(|deadline| std::time::Instant::now() >= deadline)
            {
                return Err(Box::new(BootstrapUnmountFailure {
                    diagnostics: io::Error::new(
                        io::ErrorKind::TimedOut,
                        "bootstrap unmount deadline expired",
                    )
                    .into(),
                    unmounting: self,
                }));
            }
            let next = match self.pending {
                Pending::OutputReady(BootstrapVolumes { tool, output }) => {
                    Self::after_output(tool, output.unmount())
                }
                Pending::OutputRetry { tool, output } => Self::after_output(tool, output.retry()),
                Pending::ToolReady { tool, output } => Self::after_tool(tool.unmount(), output),
                Pending::ToolRetry { tool, output } => Self::after_tool(tool.retry(), output),
                Pending::Complete(volumes) => return Ok(volumes),
            };
            match next {
                Ok(pending) => self.pending = pending,
                Err(failure) => {
                    let (diagnostics, pending) = *failure;
                    return Err(Box::new(BootstrapUnmountFailure {
                        diagnostics,
                        unmounting: Self {
                            pending,
                            deadline: self.deadline,
                        },
                    }));
                }
            }
        }
    }
    fn after_output(
        tool: Volume<'n, 'o, 'w, Mounted>,
        output: Result<Volume<'n, 'o, 'w, Unmounted>, VolumeUnmountFailure<'n, 'o, 'w>>,
    ) -> Result<Pending<'n, 'o, 'w>, Box<(FailureDiagnostics, Pending<'n, 'o, 'w>)>> {
        match output {
            Ok(output) => Ok(Pending::ToolReady { tool, output }),
            Err(output) => Err(Box::new((
                output.diagnostics().latest_only(),
                Pending::OutputRetry { tool, output },
            ))),
        }
    }
    fn after_tool(
        tool: Result<Volume<'n, 'o, 'w, Unmounted>, VolumeUnmountFailure<'n, 'o, 'w>>,
        output: Volume<'n, 'o, 'w, Unmounted>,
    ) -> Result<Pending<'n, 'o, 'w>, Box<(FailureDiagnostics, Pending<'n, 'o, 'w>)>> {
        match tool {
            Ok(tool) => Ok(Pending::Complete(BootstrapVolumes { tool, output })),
            Err(tool) => Err(Box::new((
                tool.diagnostics().latest_only(),
                Pending::ToolRetry { tool, output },
            ))),
        }
    }
}
