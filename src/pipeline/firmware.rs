// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    execution::HelperProgram,
    input::ResolvedBuild,
    process::ChildReaper,
    recipe::Task,
    step::{CommandSandbox, SetupBudget},
    storage::{Created, Mapping, Mounted, Runtime, Unmounted, Volume, Workspace},
};
use std::num::NonZeroU64;

#[derive(Debug)]
pub(super) struct BuildTools<'s, 'w> {
    pub(super) active: crate::storage::ReadOnlyChain<'s, 'w>,
    pub(super) root: Option<[u8; 32]>,
    pub(super) reaper: ChildReaper,
}
impl<'s, 'w> BuildTools<'s, 'w> {
    pub(super) fn run(
        self,
        build: &ResolvedBuild,
        task: &Task,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        artifacts: &mut super::artifacts::Artifacts<'w>,
    ) -> anyhow::Result<BuiltArtifacts<'w>> {
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
            .map(|n| crate::network::CommandNetworking::new(n, workspace));
        let completed = sandbox.directives_with_inputs::<Volume<'_, '_, '_, Mounted>>(
            volume,
            reaper,
            task.directives(),
            crate::step::DirectiveInputs {
                context: build.captured().context(),
                artifacts,
                carapace: root.as_ref(),
                network: networking.as_ref(),
            },
        )?;
        let (volume, reaper) = completed.into_parts();
        let budget = SetupBudget::begin();
        let mut mounted = volume.mount_ext4(paths.volumes())?;
        let captured = (|| -> anyhow::Result<_> {
            let pair = task
                .firmware()
                .map(|f| mounted.extract_firmware(f, workspace, &budget))
                .transpose()?;
            let mut outputs = Vec::new();
            for (name, path) in task.outputs() {
                outputs.push((
                    name.clone(),
                    mounted.extract_artifact(path.as_ref(), workspace, &budget)?,
                ));
            }
            Ok((pair, outputs))
        })();
        let cleanup_budget = SetupBudget::begin();
        let cleanup = (|| -> anyhow::Result<()> {
            mounted.unmount()?.into_snapshot().close()?.detach()?;
            Ok(())
        })()
        .and_then(|()| active.close().map_err(anyhow::Error::from))
        .and_then(|()| cleanup_budget.check().map_err(anyhow::Error::from));
        let (pair, outputs) = match (captured, cleanup) {
            (Ok(value), Ok(())) => value,
            (Err(e), Ok(())) | (Ok(_), Err(e)) => return Err(e),
            (Err(e), Err(cleanup)) => {
                return Err(e.context(format!("cleanup also failed: {cleanup:#}")));
            }
        };
        artifacts.extend(outputs);
        // Custom boot is captured byte-for-byte. Automatic binding is performed
        // when extend or final image assembly selects a carapace.
        let firmware = pair
            .map(|p| p.finalize(&crate::recipe::TransformSelection::Authored(vec![]), None))
            .transpose()?;
        Ok(BuiltArtifacts { firmware, reaper })
    }
}

#[derive(Debug)]
pub(super) struct BuiltArtifacts<'w> {
    pub(super) firmware: Option<crate::firmware::FinalizedFirmware<'w>>,
    pub(super) reaper: ChildReaper,
}
