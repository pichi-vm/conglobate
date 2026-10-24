// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::BuildError;
use crate::{
    execution::HelperProgram,
    input::ResolvedBuild,
    process::ChildReaper,
    recipe::Directive,
    step::CommandSandbox,
    storage::{Created, Mapping, Mounted, Runtime, SealedScute, Unmounted, Volume, Workspace},
};
use std::{num::NonZeroU64, sync::Arc};

/// Successful retained layers and their live read-only chain, ready for the next
/// stage. The reaper transfers with the build instead of being reinstalled.
#[derive(Debug)]
pub(super) struct DerivedCarapace<'s, 'w> {
    pub(super) active: crate::storage::ReadOnlyChain<'s, 'w>,
    pub(super) scutes: Vec<Arc<SealedScute<'w>>>,
    pub(super) reaper: ChildReaper,
}
impl<'s, 'w> DerivedCarapace<'s, 'w> {
    pub(super) fn derive(
        self,
        build: &ResolvedBuild,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        directives: &[Directive],
    ) -> Result<Self, BuildError> {
        if directives.is_empty() {
            return Ok(self);
        }
        let Self {
            active,
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
        let snapshot = Mapping::<Created>::create()?
            .snapshot(
                &active,
                scratch.attach().map_err(BuildError::from)?,
                NonZeroU64::new(crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap(),
            )
            .map_err(BuildError::from)?;
        let volume = Volume::<Unmounted>::in_init_namespace(snapshot, runtime.recovery())
            .map_err(BuildError::from)?;
        let networking = build
            .captured()
            .networking()
            .map(|resolver| crate::network::CommandNetworking::new(resolver, workspace));
        let completed = sandbox
            .directives_with_network::<Volume<'_, '_, '_, Mounted>>(
                volume,
                reaper,
                build.captured().context(),
                directives,
                None,
                networking.as_ref(),
            )
            .map_err(|failure| (*failure).reject())?;
        let (volume, reaper) = completed.into_parts();
        let scute = Arc::new(
            volume
                .into_snapshot()
                .close()
                .map_err(BuildError::from)?
                .detach()
                .map_err(BuildError::from)?
                .seal_snapshot(*active.root())
                .map_err(BuildError::from)?,
        );
        let active = active
            .extend(Arc::clone(&scute))
            .map_err(BuildError::from)?;
        produced.push(scute);
        Ok(DerivedCarapace {
            active,
            scutes: produced,
            reaper,
        })
    }
}
