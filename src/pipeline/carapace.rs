// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    execution::HelperProgram,
    input::ResolvedBuild,
    plan::Stage,
    process::ChildReaper,
    step::CommandSandbox,
    storage::{Created, Mapping, Mounted, Runtime, SealedScute, Unmounted, Volume, Workspace},
};
use std::{num::NonZeroU64, sync::Arc};

/// Retained layers and the build's reaper. Stage results keep no live mappings;
/// a dependent stage activates the complete persisted chain when it needs it.
#[derive(Debug)]
pub(super) struct DerivedCarapace<'w> {
    pub(super) scutes: Vec<Arc<SealedScute<'w>>>,
    pub(super) reaper: ChildReaper,
}
impl<'w> DerivedCarapace<'w> {
    pub(super) fn derive(
        self,
        build: &ResolvedBuild,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        stage: &Stage,
        sources: &super::sources::Sources<'_, '_>,
        active: crate::storage::ReadOnlyChain<'w>,
    ) -> anyhow::Result<Self> {
        let Self {
            scutes: mut produced,
            reaper,
        } = self;
        let paths = runtime.paths()?;
        let mappings = build.captured().execution_mappings()?;
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            mappings.uid(),
            mappings.gid(),
        );
        let scratch = workspace.snapshot_store()?;
        let snapshot = Mapping::<Created>::create()?.snapshot(
            &active,
            scratch.attach()?,
            NonZeroU64::new(carapace::ZERO_SIZE_SECTORS).unwrap(),
        )?;
        let volume = Volume::<Unmounted>::in_init_namespace(snapshot, runtime.recovery())?
            .with_mounts(&stage.mount, paths.volumes(), false)?;
        let networking = build
            .captured()
            .networking()
            .map(|resolver| crate::network::CommandNetworking::new(resolver, workspace));
        let completed = sandbox.directives_with_inputs::<Volume<'_, '_, '_, Mounted>>(
            volume,
            reaper,
            &stage.derive,
            crate::step::DirectiveInputs {
                context: build.captured().context(),
                sources,
                network: networking.as_ref(),
            },
        )?;
        let (volume, reaper) = completed.into_parts();
        let scute = Arc::new(
            volume
                .into_snapshot()?
                .close()?
                .detach()?
                .seal_snapshot(*active.root())?,
        );
        active.close()?;
        produced.push(scute);
        Ok(DerivedCarapace {
            scutes: produced,
            reaper,
        })
    }
}
