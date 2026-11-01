// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::rejection::DiscardStorage;
use crate::{
    execution::HelperProgram,
    formatter::{CapturedFormatter, Ext4Format},
    input::ResolvedBuild,
    plan::Bootstrap,
    process::ChildReaper,
    step::{CommandSandbox, SetupBudget},
    storage::{BootstrapVolumes, Created, Mapping, Mounted, Runtime, Unmounted, Volume, Workspace},
};
use anyhow::Context;
use std::{num::NonZeroU64, sync::Arc};

#[derive(Debug)]
pub(super) struct BootstrapBuild<'a> {
    build: &'a ResolvedBuild,
    stage: &'a Bootstrap,
    directives: &'a [crate::plan::Directive],
    artifacts: &'a super::artifacts::Artifacts<'a>,
}
impl<'a> BootstrapBuild<'a> {
    pub(super) fn new(
        build: &'a ResolvedBuild,
        stage: &'a Bootstrap,
        directives: &'a [crate::plan::Directive],
        artifacts: &'a super::artifacts::Artifacts<'a>,
    ) -> Self {
        Self {
            build,
            stage,
            directives,
            artifacts,
        }
    }
    pub(super) fn execute<'w>(
        &self,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        active: crate::storage::ReadOnlyChain<'w>,
        reaper: ChildReaper,
        carapace: Option<&[u8; 32]>,
    ) -> anyhow::Result<Bootstrapped<'w>> {
        let captured = CapturedFormatter::for_filesystem(self.stage.filesystem())
            .context("capture trusted formatter")?;
        self.produce(
            workspace,
            runtime,
            captured.stage(workspace)?,
            active,
            reaper,
            carapace,
        )
    }

    fn produce<'w>(
        &self,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        formatter: crate::formatter::FormatterRuntime,
        active: crate::storage::ReadOnlyChain<'w>,
        reaper: ChildReaper,
        carapace: Option<&[u8; 32]>,
    ) -> anyhow::Result<Bootstrapped<'w>> {
        let sectors = NonZeroU64::new(carapace::ZERO_SIZE_SECTORS).unwrap();
        let origin = Mapping::<Created>::create()?.zero(sectors)?;
        let output = Mapping::<Created>::create()?.snapshot(
            &origin,
            workspace.snapshot_store()?.attach()?,
            sectors,
        )?;
        let formatted = self
            .format(formatter, output)
            .context("format bootstrap filesystem")?;
        let (formatter, output) = formatted.into_parts()?;
        formatter.close()?;
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
        let completed = sandbox.directives_with_inputs::<BootstrapVolumes<'_, '_, '_, Mounted>>(
            BootstrapVolumes::new(tool, output),
            reaper,
            self.directives,
            crate::step::DirectiveInputs {
                context: self.build.captured().context(),
                artifacts: self.artifacts,
                carapace,
                network: networking.as_ref(),
            },
        )?;
        let (volumes, reaper) = completed.into_parts();
        let (tool, output) = volumes.into_parts();
        tool.discard()?;
        let scute = Arc::new(
            output
                .into_snapshot()?
                .close()?
                .detach()?
                .seal_snapshot([0; 32])?,
        );
        active.close()?;
        origin.close()?;
        Ok(Bootstrapped { scute, reaper })
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
    pub(super) reaper: ChildReaper,
    pub(super) scute: Arc<crate::storage::SealedScute<'w>>,
}
