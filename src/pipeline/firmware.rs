// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::BuildError;
use crate::{
    execution::HelperProgram,
    input::ResolvedBuild,
    isolation::IdRange,
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
    ) -> Result<crate::firmware::FinalizedFirmware<'w>, BuildError> {
        let Self {
            active,
            root,
            reaper,
        } = self;
        let snapshot = Mapping::<Created>::create()?
            .snapshot(
                &active,
                workspace
                    .snapshot_store()?
                    .attach()
                    .map_err(BuildError::from)?,
                NonZeroU64::new(crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap(),
            )
            .map_err(BuildError::from)?;
        let volume = Volume::<Unmounted>::in_init_namespace(snapshot, runtime.recovery())
            .map_err(BuildError::from)?;
        let paths = runtime.paths()?;
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
        );
        let networking = build
            .captured()
            .networking()
            .map(|resolver| crate::network::CommandNetworking::new(resolver, workspace));
        let completed = sandbox
            .directives_with_network::<Volume<'_, '_, '_, Mounted>>(
                volume,
                reaper,
                build.captured().context(),
                stage.directives(),
                root.as_ref(),
                networking.as_ref(),
            )
            .map_err(|failure| (*failure).reject())?;
        let (volume, _reaper) = completed.into_parts();
        let budget = SetupBudget::begin();
        let mut mounted = volume
            .mount_ext4(paths.volumes())
            .map_err(BuildError::from)?;
        let captured = mounted.extract_firmware(stage, workspace, &budget);
        let cleanup_budget = SetupBudget::begin();
        let cleanup = (|| -> Result<(), BuildError> {
            mounted
                .unmount()
                .map_err(BuildError::from)?
                .into_snapshot()
                .close()
                .map_err(BuildError::from)?
                .detach()
                .map_err(BuildError::from)?;
            Ok(())
        })()
        .and_then(|()| active.close().map_err(BuildError::from))
        .and_then(|()| cleanup_budget.check().map_err(BuildError::from));
        let captured = match (captured.map_err(BuildError::from), cleanup) {
            (Ok(captured), Ok(())) => captured,
            (Err(error), Ok(())) | (Ok(_), Err(error)) => return Err(error),
            (Err(error), Err(cleanup)) => {
                return Err(error.with_cleanup(Err(cleanup)));
            }
        };
        let root = root.map(hex::encode);
        captured
            .finalize(
                build.captured().plan().recipe().transform(),
                root.as_deref(),
            )
            .map_err(BuildError::from)
    }
}
