// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{BuildError, rejection::DiscardStorage};
use crate::{
    execution::HelperProgram,
    formatter::{Ext4Format, FormatterManifest},
    input::{CheckedImage, ResolvedBuild},
    isolation::IdRange,
    process::ChildReaper,
    recipe::Bootstrap,
    request::BuildRequest,
    step::{CommandSandbox, SetupBudget},
    storage::{BootstrapVolumes, Created, Mapping, Mounted, Runtime, Unmounted, Volume, Workspace},
};
use std::{num::NonZeroU64, sync::Arc};

#[derive(Debug)]
pub(super) struct BootstrapBuild<'a> {
    build: &'a ResolvedBuild,
    stage: &'a Bootstrap,
    tools: &'a CheckedImage,
}
impl<'a> BootstrapBuild<'a> {
    pub(super) fn new(build: &'a ResolvedBuild) -> Result<Self, BuildError> {
        let recipe = build.captured().plan().recipe();
        let stage = recipe
            .bootstrap()
            .ok_or(BuildError::Unsupported("missing bootstrap stage"))?;
        let tools = build
            .images()
            .get(stage.from())
            .ok_or(BuildError::Unsupported("missing bootstrap tools"))?;
        if tools.chain().is_none() {
            return Err(BuildError::Unsupported(
                "bootstrap tools require a root volume",
            ));
        }
        Ok(Self {
            build,
            stage,
            tools,
        })
    }
    pub(super) fn execute<'w>(
        &self,
        request: &BuildRequest,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
    ) -> Result<Bootstrapped<'w>, BuildError> {
        let manifest = request.formatter_runtime().ok_or_else(|| {
            BuildError::InvalidExecution("bootstrap requires --formatter-runtime")
        })?;
        let captured = FormatterManifest::capture(manifest)?;
        match captured.stage(workspace) {
            Ok(formatter) => self.produce(workspace, runtime, formatter),
            Err(failure) => {
                let (primary, cleanup) = failure.cleanup_with_source();
                Err(BuildError::FormatterStaging(primary)
                    .with_cleanup(cleanup.map_err(BuildError::from)))
            }
        }
    }
    fn produce<'w>(
        &self,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        formatter: crate::formatter::FormatterRuntime,
    ) -> Result<Bootstrapped<'w>, BuildError> {
        let sectors =
            NonZeroU64::new(crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap();
        let origin = Mapping::<Created>::create()?
            .zero(sectors)
            .map_err(BuildError::from)?;
        let output = Mapping::<Created>::create()?
            .snapshot(
                &origin,
                workspace
                    .snapshot_store()?
                    .attach()
                    .map_err(BuildError::from)?,
                sectors,
            )
            .map_err(BuildError::from)?;
        let formatted = self.format(formatter, output)?;
        let (formatter, output, receipt) = formatted.into_recorded_parts();
        formatter.close().map_err(BuildError::from)?;
        let active = self
            .tools
            .chain()
            .expect("validated tools")
            .assemble(self.build.cache(), workspace)
            .map_err(BuildError::from)?;
        let tool = Mapping::<Created>::create()?
            .snapshot(
                &active,
                workspace
                    .snapshot_store()?
                    .attach()
                    .map_err(BuildError::from)?,
                sectors,
            )
            .map_err(BuildError::from)?;
        let tool = Volume::<Unmounted>::in_init_namespace(tool, runtime.recovery())
            .map_err(BuildError::from)?;
        let output = Volume::<Unmounted>::in_init_namespace(output, runtime.recovery())
            .map_err(BuildError::from)?;
        let paths = runtime.paths()?;
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
        );
        let networking = self
            .build
            .captured()
            .networking()
            .map(|resolver| crate::network::CommandNetworking::new(resolver, workspace));
        let completed = sandbox
            .directives_with_network::<BootstrapVolumes<'_, '_, '_, Mounted>>(
                BootstrapVolumes::new(tool, output),
                ChildReaper::install()?,
                self.build.captured().context(),
                self.stage.directives(),
                None,
                networking.as_ref(),
            )
            .map_err(|failure| (*failure).reject())?;
        let (volumes, reaper) = completed.into_parts();
        let (tool, output) = volumes.into_parts();
        tool.discard()?;
        let scute = Arc::new(
            output
                .into_snapshot()
                .close()
                .map_err(BuildError::from)?
                .detach()
                .map_err(BuildError::from)?
                .seal_snapshot([0; 32])
                .map_err(BuildError::from)?,
        );
        active.close().map_err(BuildError::from)?;
        origin.close().map_err(BuildError::from)?;
        Ok(Bootstrapped {
            scute,
            reaper,
            receipt,
        })
    }
    fn format<'o, 'w>(
        &self,
        formatter: crate::formatter::FormatterRuntime,
        snapshot: crate::storage::SnapshotMapping<'o, 'w>,
    ) -> Result<crate::formatter::FormattedSnapshot<'o, 'w>, BuildError> {
        let helper = HelperProgram::current()?;
        let running = match formatter.format(
            snapshot,
            Ext4Format::from(self.stage.filesystem()),
            &helper,
            SetupBudget::begin(),
        ) {
            Ok(running) => running,
            Err(failure) => {
                let (error, formatter, snapshot) = failure.into_parts();
                return Err(Self::reject_format(error, formatter, snapshot));
            }
        };
        match running.wait() {
            Ok(done) => Ok(done),
            Err(crate::formatter::FormatFailure::Completion(failure)) => {
                let (error, formatter, snapshot) = failure.into_parts();
                Err(Self::reject_format(error, formatter, snapshot))
            }
            Err(crate::formatter::FormatFailure::Wait(failure)) => match failure.cancel() {
                Ok((error, formatter, snapshot)) => {
                    Err(Self::reject_format(error, formatter, snapshot))
                }
                Err(failure) => Err(BuildError::FormatterWait(failure.diagnostics().clone())),
            },
        }
    }
    fn reject_format(
        error: impl Into<crate::storage::FailureDiagnostics>,
        formatter: crate::formatter::FormatterRuntime,
        snapshot: crate::storage::SnapshotMapping<'_, '_>,
    ) -> BuildError {
        let storage = snapshot
            .close()
            .map_err(BuildError::from)
            .and_then(|closed| closed.detach().map_err(BuildError::from));
        let runtime = formatter.close().map_err(BuildError::from);
        BuildError::Formatter(error.into())
            .with_cleanup(storage.map(|_| ()))
            .with_cleanup(runtime)
    }
}

/// Bootstrap output only: tools have been discarded, and no later stage ran.
#[derive(Debug)]
pub(super) struct Bootstrapped<'w> {
    pub(super) scute: Arc<crate::storage::SealedScute<'w>>,
    pub(super) reaper: ChildReaper,
    pub(super) receipt: crate::formatter::FormatReceipt,
}
