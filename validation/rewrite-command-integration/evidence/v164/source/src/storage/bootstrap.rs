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
    Output {
        tool: Volume<'n, 'o, 'w, Mounted>,
        output: VolumeUnmountFailure<'n, 'o, 'w>,
    },
    Tool {
        tool: VolumeUnmountFailure<'n, 'o, 'w>,
        output: Volume<'n, 'o, 'w, Unmounted>,
    },
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
    pending: Pending<'n, 'o, 'w>,
}
impl<'n, 'o, 'w> BootstrapVolumes<'n, 'o, 'w, Mounted> {
    pub fn unmount(
        self,
    ) -> Result<BootstrapVolumes<'n, 'o, 'w, Unmounted>, Box<BootstrapUnmountFailure<'n, 'o, 'w>>>
    {
        let Self { tool, output } = self;
        match output.unmount() {
            Ok(output) => Self::finish_tool(tool.unmount(), output),
            Err(output) => Err(Box::new(BootstrapUnmountFailure {
                diagnostics: output.diagnostics().latest_only(),
                pending: Pending::Output { tool, output },
            })),
        }
    }
    fn finish_tool(
        tool: Result<Volume<'n, 'o, 'w, Unmounted>, VolumeUnmountFailure<'n, 'o, 'w>>,
        output: Volume<'n, 'o, 'w, Unmounted>,
    ) -> Result<BootstrapVolumes<'n, 'o, 'w, Unmounted>, Box<BootstrapUnmountFailure<'n, 'o, 'w>>>
    {
        match tool {
            Ok(tool) => Ok(BootstrapVolumes { tool, output }),
            Err(tool) => Err(Box::new(BootstrapUnmountFailure {
                diagnostics: tool.diagnostics().latest_only(),
                pending: Pending::Tool { tool, output },
            })),
        }
    }
}
impl<'n, 'o, 'w> BootstrapUnmountFailure<'n, 'o, 'w> {
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    pub fn retry(self) -> Result<BootstrapVolumes<'n, 'o, 'w, Unmounted>, Box<Self>> {
        let result = match self.pending {
            Pending::Output { tool, output } => match output.retry() {
                Ok(output) => BootstrapVolumes::<Mounted>::finish_tool(tool.unmount(), output),
                Err(output) => Err(Box::new(Self {
                    diagnostics: output.diagnostics().latest_only(),
                    pending: Pending::Output { tool, output },
                })),
            },
            Pending::Tool { tool, output } => {
                BootstrapVolumes::<Mounted>::finish_tool(tool.retry(), output)
            }
        };
        result.map_err(|failure| {
            Box::new(Self {
                diagnostics: self.diagnostics.followed_by(failure.diagnostics),
                pending: failure.pending,
            })
        })
    }
}
