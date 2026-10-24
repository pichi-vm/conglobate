// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Mounted, Unmounted, Volume, VolumeState};
use std::io;

/// The disposable tool root and persistent output have one lifetime owner.
/// State denotes both volumes. Partial finalization never yields build output.
#[derive(Debug)]
pub(crate) struct BootstrapVolumes<'ns, 'origin, 'workspace, S: VolumeState> {
    tool: Volume<'ns, 'origin, 'workspace, S>,
    output: Volume<'ns, 'origin, 'workspace, S>,
}
impl<'n, 'o, 'w, S: VolumeState> BootstrapVolumes<'n, 'o, 'w, S> {
    pub(crate) fn new(tool: Volume<'n, 'o, 'w, S>, output: Volume<'n, 'o, 'w, S>) -> Self {
        Self { tool, output }
    }
}
impl<'n, 'o, 'w> BootstrapVolumes<'n, 'o, 'w, Unmounted> {
    pub(crate) fn into_parts(
        self,
    ) -> (Volume<'n, 'o, 'w, Unmounted>, Volume<'n, 'o, 'w, Unmounted>) {
        (self.tool, self.output)
    }
    pub(crate) fn validate_snapshots(&self) -> io::Result<()> {
        self.tool.validate_snapshot()?;
        self.output.validate_snapshot()
    }
}
impl<'n, 'o, 'w> BootstrapVolumes<'n, 'o, 'w, Unmounted> {
    pub(crate) fn mount_ext4_until(
        self,
        parent: &std::path::Path,
        deadline: std::time::Instant,
    ) -> anyhow::Result<BootstrapVolumes<'n, 'o, 'w, Mounted>> {
        if std::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "bootstrap mount deadline expired",
            )
            .into());
        }
        let tool = self.tool.mount_ext4(parent)?;
        let output = match self.output.mount_ext4(parent) {
            Ok(output) => output,
            Err(error) => {
                return Err(match tool.unmount() {
                    Ok(_) => anyhow::Error::new(error),
                    Err(cleanup) => {
                        anyhow::Error::new(error).context(format!("tool mount cleanup: {cleanup}"))
                    }
                });
            }
        };
        let volumes = BootstrapVolumes { tool, output };
        if std::time::Instant::now() >= deadline {
            let error = io::Error::new(io::ErrorKind::TimedOut, "bootstrap mount deadline expired");
            return Err(
                match volumes
                    .unmount_until(std::time::Instant::now() + std::time::Duration::from_secs(30))
                {
                    Ok(_) => anyhow::Error::new(error),
                    Err(cleanup) => anyhow::Error::new(error)
                        .context(format!("bootstrap mount cleanup: {cleanup}")),
                },
            );
        }
        Ok(volumes)
    }
}
impl<'n, 'o, 'w> BootstrapVolumes<'n, 'o, 'w, Mounted> {
    /// COPY sees the tool root at / and the output root at /mnt, including
    /// symlinks that cross that boundary. Both volumes remain exclusively owned.
    pub(crate) fn copy(
        &mut self,
        context: &crate::context::CapturedContext,
        directive: &crate::recipe::Copy,
    ) -> Result<(), crate::copy::CopyError> {
        self.tool
            .copy_with_output(&mut self.output, context, directive)
    }

    pub(crate) fn tool(&self) -> &Volume<'n, 'o, 'w, Mounted> {
        &self.tool
    }
    pub(crate) fn output(&self) -> &Volume<'n, 'o, 'w, Mounted> {
        &self.output
    }

    pub(crate) fn unmount_until(
        self,
        deadline: std::time::Instant,
    ) -> io::Result<BootstrapVolumes<'n, 'o, 'w, Unmounted>> {
        if std::time::Instant::now() >= deadline {
            std::mem::forget(self);
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "bootstrap unmount deadline expired",
            ));
        }
        let output = match self.output.unmount() {
            Ok(output) => output,
            Err(error) => {
                std::mem::forget(self.tool);
                return Err(error);
            }
        };
        if std::time::Instant::now() >= deadline {
            std::mem::forget(self.tool);
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "bootstrap unmount deadline expired",
            ));
        }
        let tool = self.tool.unmount()?;
        if std::time::Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "bootstrap unmount deadline expired",
            ));
        }
        Ok(BootstrapVolumes { tool, output })
    }
}
