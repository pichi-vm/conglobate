// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    execution::HelperProgram,
    input::ResolvedBuild,
    process::ChildReaper,
    recipe::Firmware,
    step::{CommandSandbox, SetupBudget},
    storage::{Created, Mapping, Mounted, Runtime, Unmounted, Volume, Workspace},
};
use std::num::NonZeroU64;

/// A disposable tool chain plus the separately selected output-root identity.
/// Consuming execution cannot turn firmware writes into retained scutes.
#[derive(Debug)]
pub(super) struct FirmwareTools<'s, 'w> {
    pub(super) active: crate::storage::ReadOnlyChain<'s, 'w>,
    pub(super) root: Option<[u8; 32]>,
    pub(super) reaper: ChildReaper,
}
impl<'s, 'w> FirmwareTools<'s, 'w> {
    pub(super) fn run(
        self,
        build: &ResolvedBuild,
        stage: &Firmware,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
    ) -> anyhow::Result<crate::firmware::ExtractedFirmware<'w>> {
        let Self {
            active,
            root,
            reaper,
        } = self;
        let snapshot = Mapping::<Created>::create()?.snapshot(
            &active,
            workspace.snapshot_store()?.attach()?,
            NonZeroU64::new(crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap(),
        )?;
        let volume = Volume::<Unmounted>::in_init_namespace(snapshot, runtime.recovery())?;
        let paths = runtime.paths()?;
        let mappings = build.captured().execution_mappings()?;
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            mappings.uid(),
            mappings.gid(),
        );
        let networking = build
            .captured()
            .networking()
            .map(|resolver| crate::network::CommandNetworking::new(resolver, workspace));
        let completed = sandbox.directives_with_network::<Volume<'_, '_, '_, Mounted>>(
            volume,
            reaper,
            build.captured().context(),
            stage.directives(),
            root.as_ref(),
            networking.as_ref(),
        )?;
        let (volume, _reaper) = completed.into_parts();
        let budget = SetupBudget::begin();
        let mut mounted = volume.mount_ext4(paths.volumes())?;
        let captured = mounted.extract_firmware(stage, workspace, &budget);
        let cleanup_budget = SetupBudget::begin();
        let cleanup = (|| -> anyhow::Result<()> {
            mounted.unmount()?.into_snapshot().close()?.detach()?;
            Ok(())
        })()
        .and_then(|()| active.close().map_err(anyhow::Error::from))
        .and_then(|()| cleanup_budget.check().map_err(anyhow::Error::from));
        let captured = match (captured.map_err(anyhow::Error::from), cleanup) {
            (Ok(captured), Ok(())) => captured,
            (Err(error), Ok(())) | (Ok(_), Err(error)) => return Err(error),
            (Err(error), Err(cleanup)) => {
                return Err(error.context(format!("cleanup also failed: {cleanup:#}")));
            }
        };
        Ok(captured)
    }
}
