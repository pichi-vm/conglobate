// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::rejection::DiscardStorage;
use crate::{
    execution::HelperProgram,
    formatter::{Ext4Format, FormatterManifest},
    input::{CheckedImage, ResolvedBuild},
    process::ChildReaper,
    recipe::Bootstrap,
    request::BuildRequest,
    step::{CommandSandbox, SetupBudget},
    storage::{BootstrapVolumes, Created, Mapping, Mounted, Runtime, Unmounted, Volume, Workspace},
};
use anyhow::anyhow;
use std::{num::NonZeroU64, sync::Arc};

#[derive(Debug)]
pub(super) struct BootstrapBuild<'a> {
    build: &'a ResolvedBuild,
    stage: &'a Bootstrap,
    tools: &'a CheckedImage,
}
impl<'a> BootstrapBuild<'a> {
    pub(super) fn new(build: &'a ResolvedBuild) -> anyhow::Result<Self> {
        let recipe = build.captured().plan().recipe();
        let stage = recipe
            .bootstrap()
            .ok_or(anyhow!("missing bootstrap stage"))?;
        let tools = build
            .images()
            .get(stage.from())
            .ok_or(anyhow!("missing bootstrap tools"))?;
        if tools.chain().is_none() {
            return Err(anyhow!("bootstrap tools require a root volume",));
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
    ) -> anyhow::Result<Bootstrapped<'w>> {
        let manifest = request
            .formatter_runtime()
            .ok_or_else(|| anyhow!("bootstrap requires --formatter-runtime"))?;
        let captured = FormatterManifest::capture(manifest)?;
        self.produce(workspace, runtime, captured.stage(workspace)?)
    }

    fn produce<'w>(
        &self,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        formatter: crate::formatter::FormatterRuntime,
    ) -> anyhow::Result<Bootstrapped<'w>> {
        let sectors =
            NonZeroU64::new(crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap();
        let origin = Mapping::<Created>::create()?.zero(sectors)?;
        let output = Mapping::<Created>::create()?.snapshot(
            &origin,
            workspace.snapshot_store()?.attach()?,
            sectors,
        )?;
        let formatted = self.format(formatter, output)?;
        let (formatter, output, receipt) = formatted.into_recorded_parts();
        formatter.close()?;
        let active = self
            .tools
            .chain()
            .expect("validated tools")
            .assemble(self.build.cache(), workspace)?;
        let tool = Mapping::<Created>::create()?.snapshot(
            &active,
            workspace.snapshot_store()?.attach()?,
            sectors,
        )?;
        let tool = Volume::<Unmounted>::in_init_namespace(tool, runtime.recovery())?;
        let output = Volume::<Unmounted>::in_init_namespace(output, runtime.recovery())?;
        let paths = runtime.paths()?;
        let mappings = self.build.captured().execution_mappings()?;
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            mappings.uid(),
            mappings.gid(),
        );
        let networking = self
            .build
            .captured()
            .networking()
            .map(|resolver| crate::network::CommandNetworking::new(resolver, workspace));
        let completed = sandbox.directives_with_network::<BootstrapVolumes<'_, '_, '_, Mounted>>(
            BootstrapVolumes::new(tool, output),
            ChildReaper::install()?,
            self.build.captured().context(),
            self.stage.directives(),
            None,
            networking.as_ref(),
        )?;
        let (volumes, reaper) = completed.into_parts();
        let (tool, output) = volumes.into_parts();
        tool.discard()?;
        let scute = Arc::new(
            output
                .into_snapshot()
                .close()?
                .detach()?
                .seal_snapshot([0; 32])?,
        );
        active.close()?;
        origin.close()?;
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
    ) -> anyhow::Result<crate::formatter::FormattedSnapshot<'o, 'w>> {
        let helper = HelperProgram::current()?;
        formatter
            .format(
                snapshot,
                Ext4Format::from(self.stage.filesystem()),
                &helper,
                SetupBudget::begin(),
            )?
            .wait()
    }
}

/// Bootstrap output only: tools have been discarded, and no later stage ran.
#[derive(Debug)]
pub(super) struct Bootstrapped<'w> {
    pub(super) scute: Arc<crate::storage::SealedScute<'w>>,
    pub(super) reaper: ChildReaper,
    pub(super) receipt: crate::formatter::FormatReceipt,
}
