// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{BuildError, CompletedImage, carapace::CarapaceBuild};
use crate::{
    execution::HelperProgram,
    input::{CheckedImage, ResolvedBuild},
    inventory::Inventory,
    isolation::IdRange,
    output::AssembledImage,
    process::ChildReaper,
    recipe::Firmware,
    request::BuildRequest,
    step::{CommandSandbox, SetupBudget},
    storage::{Created, Mapping, Mounted, Runtime, Unmounted, Volume, Workspace},
};
use std::{num::NonZeroU64, path::Path};

#[derive(Debug)]
pub(super) struct FirmwareBuild<'a> {
    build: &'a ResolvedBuild,
    stage: &'a Firmware,
    tools: &'a CheckedImage,
    base: Option<&'a CheckedImage>,
}
impl<'a> FirmwareBuild<'a> {
    pub(super) fn new(build: &'a ResolvedBuild) -> Result<Self, BuildError> {
        let recipe = build.captured().plan().recipe();
        if recipe.bootstrap().is_some() {
            return Err(BuildError::Unsupported("bootstrap coordinator"));
        }
        let stage = recipe
            .firmware()
            .ok_or(BuildError::Unsupported("missing firmware stage"))?;
        let base = recipe
            .carapace()
            .and_then(|s| s.from())
            .and_then(|name| build.images().get(name));
        let tools = stage
            .from()
            .and_then(|name| build.images().get(name))
            .or(base)
            .ok_or(BuildError::Unsupported("missing firmware tools"))?;
        if tools.chain().is_none() {
            return Err(BuildError::Unsupported(
                "firmware tools require root volume",
            ));
        }
        Ok(Self {
            build,
            stage,
            tools,
            base,
        })
    }
    pub(super) fn execute(
        &self,
        request: &BuildRequest,
        workspace: &Workspace,
    ) -> Result<CompletedImage, BuildError> {
        let runtime = Runtime::create(workspace, Path::new("/run")).map_err(Self::error)?;
        let result = self.produce(request, workspace, &runtime);
        let cleanup = runtime.close().map_err(Self::error);
        match (result, cleanup) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => {
                Err(Self::error(format!("{error}; runtime cleanup: {cleanup}")))
            }
        }
    }
    fn produce(
        &self,
        request: &BuildRequest,
        workspace: &Workspace,
        runtime: &Runtime<'_>,
    ) -> Result<CompletedImage, BuildError> {
        let (active, produced, root, reaper) = if self
            .build
            .captured()
            .plan()
            .recipe()
            .carapace()
            .is_some_and(|stage| !stage.directives().is_empty())
        {
            let derived = CarapaceBuild::new(self.build)?.run_layers(workspace, runtime)?;
            let root = Some(*derived.active.root());
            let tools = if self.stage.from().is_some() {
                derived.active.close().map_err(Self::error)?;
                self.tools
                    .chain()
                    .expect("validated tools")
                    .assemble(self.build.cache(), workspace)
                    .map_err(Self::error)?
            } else {
                derived.active
            };
            (tools, derived.scutes, root, derived.reaper)
        } else {
            (
                self.tools
                    .chain()
                    .expect("validated tools")
                    .assemble(self.build.cache(), workspace)
                    .map_err(Self::error)?,
                Vec::new(),
                self.base.and_then(|base| base.chain()).map(|c| *c.root()),
                ChildReaper::install()?,
            )
        };
        let pair = FirmwareTools {
            active,
            root,
            reaper,
        }
        .run(self.build, self.stage, workspace, runtime)?;
        let image = AssembledImage::firmware(
            self.build.cache(),
            self.base.map(|b| b.image()),
            &pair,
            &produced,
            request.output(),
        )?;
        let inventory = Inventory::from_build(request, self.build, &image, pair.dtb(), &produced)?;
        inventory.check_output(&image)?;
        Ok(CompletedImage { image, inventory })
    }
    fn error(error: impl std::fmt::Display) -> BuildError {
        BuildError::Execution(error.to_string())
    }
}

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
                    .map_err(FirmwareBuild::error)?,
                NonZeroU64::new(crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap(),
            )
            .map_err(FirmwareBuild::error)?;
        let volume = Volume::<Unmounted>::in_init_namespace(snapshot, runtime.recovery())
            .map_err(FirmwareBuild::error)?;
        let paths = runtime.paths()?;
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
        );
        let networking = build
            .captured()
            .resolver()
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
            .map_err(|failure| CarapaceBuild::reject(*failure))?;
        let (volume, _reaper) = completed.into_parts();
        let budget = SetupBudget::begin();
        let mut mounted = volume
            .mount_ext4(paths.volumes())
            .map_err(FirmwareBuild::error)?;
        let captured = mounted.extract_firmware(stage, workspace, &budget);
        let cleanup_budget = SetupBudget::begin();
        let cleanup = (|| -> Result<(), BuildError> {
            mounted
                .unmount()
                .map_err(FirmwareBuild::error)?
                .into_snapshot()
                .close()
                .map_err(FirmwareBuild::error)?
                .detach()
                .map_err(FirmwareBuild::error)?;
            Ok(())
        })()
        .and_then(|()| active.close().map_err(FirmwareBuild::error))
        .and_then(|()| cleanup_budget.check().map_err(BuildError::from));
        let captured = match (captured.map_err(FirmwareBuild::error), cleanup) {
            (Ok(captured), Ok(())) => captured,
            (Err(error), Ok(())) | (Ok(_), Err(error)) => return Err(error),
            (Err(error), Err(cleanup)) => {
                return Err(FirmwareBuild::error(format!(
                    "{error}; firmware cleanup: {cleanup}"
                )));
            }
        };
        let root = root.map(hex::encode);
        captured
            .finalize(
                build.captured().plan().recipe().transform(),
                root.as_deref(),
            )
            .map_err(FirmwareBuild::error)
    }
}
