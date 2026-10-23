// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{BuildError, CompletedImage};
use crate::{
    execution::HelperProgram,
    formatter::{Ext4Format, FormatterManifest},
    input::{CheckedImage, ResolvedBuild},
    inventory::Inventory,
    isolation::IdRange,
    output::AssembledImage,
    process::ChildReaper,
    recipe::Bootstrap,
    request::BuildRequest,
    step::{CommandSandbox, SetupBudget, StageFailure},
    storage::{BootstrapVolumes, Created, Mapping, Mounted, Runtime, Unmounted, Volume, Workspace},
};
use std::{num::NonZeroU64, path::Path, sync::Arc};

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
    pub(super) fn execute(
        &self,
        request: &BuildRequest,
        workspace: &Workspace,
    ) -> Result<CompletedImage, BuildError> {
        let manifest = request
            .formatter_runtime()
            .ok_or_else(|| Self::error("bootstrap requires --formatter-runtime"))?;
        let captured = FormatterManifest::capture(manifest)?;
        let runtime = Runtime::create(workspace, Path::new("/run")).map_err(Self::error)?;
        let result = match captured.stage(workspace) {
            Ok(formatter) => self.produce(request, workspace, &runtime, formatter),
            Err(failure) => {
                let primary = failure.to_string();
                Err(match failure.cleanup() {
                    Ok(()) => Self::error(primary),
                    Err(cleanup) => {
                        Self::error(format!("{primary}; formatter staging cleanup: {cleanup}"))
                    }
                })
            }
        };
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
        formatter: crate::formatter::FormatterRuntime,
    ) -> Result<CompletedImage, BuildError> {
        let sectors =
            NonZeroU64::new(crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512).unwrap();
        let origin = Mapping::<Created>::create()?
            .zero(sectors)
            .map_err(Self::error)?;
        let output = Mapping::<Created>::create()?
            .snapshot(
                &origin,
                workspace.snapshot_store()?.attach().map_err(Self::error)?,
                sectors,
            )
            .map_err(Self::error)?;
        let formatted = self.format(formatter, output)?;
        let (formatter, output, receipt) = formatted.into_recorded_parts();
        formatter.close().map_err(Self::error)?;
        let active = self
            .tools
            .chain()
            .expect("validated tools")
            .assemble(self.build.cache(), workspace)
            .map_err(Self::error)?;
        let tool = Mapping::<Created>::create()?
            .snapshot(
                &active,
                workspace.snapshot_store()?.attach().map_err(Self::error)?,
                sectors,
            )
            .map_err(Self::error)?;
        let tool = Volume::<Unmounted>::in_init_namespace(tool, runtime.recovery())
            .map_err(Self::error)?;
        let output = Volume::<Unmounted>::in_init_namespace(output, runtime.recovery())
            .map_err(Self::error)?;
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
            .resolver()
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
            .map_err(|failure| Self::reject(*failure))?;
        let (volumes, reaper) = completed.into_parts();
        let (tool, output) = volumes.into_parts();
        Self::discard(tool)?;
        let scute = Arc::new(
            output
                .into_snapshot()
                .close()
                .map_err(Self::error)?
                .detach()
                .map_err(Self::error)?
                .seal_snapshot([0; 32])
                .map_err(Self::error)?,
        );
        active.close().map_err(Self::error)?;
        origin.close().map_err(Self::error)?;
        let produced = vec![scute];
        let recipe = self.build.captured().plan().recipe();
        let directives = recipe
            .carapace()
            .map(|stage| stage.directives())
            .unwrap_or(&[]);
        let (active, produced, reaper, root) = if directives.is_empty() {
            let root = *produced[0].root();
            (None, produced, reaper, root)
        } else {
            let derived = super::carapace::DerivedCarapace {
                active: crate::storage::ReadOnlyChain::from_root(Arc::clone(&produced[0]))
                    .map_err(Self::error)?,
                scutes: produced,
                reaper,
            }
            .derive(self.build, workspace, runtime, directives)?;
            let root = *derived.active.root();
            (Some(derived.active), derived.scutes, derived.reaper, root)
        };
        let (image, inventory) = if let Some(stage) = recipe.firmware() {
            let tools = if let Some(reference) = stage.from() {
                if let Some(active) = active {
                    active.close().map_err(Self::error)?;
                }
                let tools = self
                    .build
                    .images()
                    .get(reference)
                    .ok_or_else(|| Self::error("missing resolved firmware tools"))?;
                tools
                    .chain()
                    .ok_or_else(|| Self::error("firmware tools require a root volume"))?
                    .assemble(self.build.cache(), workspace)
                    .map_err(Self::error)?
            } else {
                match active {
                    Some(active) => active,
                    None => crate::storage::ReadOnlyChain::from_root(Arc::clone(&produced[0]))
                        .map_err(Self::error)?,
                }
            };
            let pair = super::firmware::FirmwareTools {
                active: tools,
                root: Some(root),
                reaper,
            }
            .run(self.build, stage, workspace, runtime)?;
            let image = AssembledImage::firmware(
                self.build.cache(),
                None,
                &pair,
                &produced,
                request.output(),
            )?;
            let inventory =
                Inventory::from_build(request, self.build, &image, pair.dtb(), &produced)?;
            (image, inventory)
        } else {
            if let Some(active) = active {
                active.close().map_err(Self::error)?;
            }
            let root = hex::encode(root);
            let transformed = recipe.transform().apply(None, Some(&root))?;
            let image = AssembledImage::bootstrap_layers(&produced, request.output())?;
            let inventory =
                Inventory::from_build(request, self.build, &image, &transformed, &produced)?;
            (image, inventory)
        };
        let inventory = inventory.with_formatter(receipt)?;
        inventory.check_output(&image)?;
        Ok(CompletedImage { image, inventory })
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
                Err(failure) => Err(Self::error(failure)),
            },
        }
    }
    fn reject_format(
        error: impl std::fmt::Display,
        formatter: crate::formatter::FormatterRuntime,
        snapshot: crate::storage::SnapshotMapping<'_, '_>,
    ) -> BuildError {
        let storage = snapshot
            .close()
            .map_err(Self::error)
            .and_then(|closed| closed.detach().map_err(Self::error));
        let runtime = formatter.close();
        let mut diagnostic = error.to_string();
        if let Err(error) = storage {
            diagnostic.push_str(&format!("; formatter storage cleanup: {error}"));
        }
        if let Err(error) = runtime {
            diagnostic.push_str(&format!("; formatter runtime cleanup: {error}"));
        }
        Self::error(diagnostic)
    }
    fn error(error: impl std::fmt::Display) -> BuildError {
        BuildError::Execution(error.to_string())
    }
    fn discard(volume: Volume<'_, '_, '_, Unmounted>) -> Result<(), BuildError> {
        volume
            .into_snapshot()
            .close()
            .map_err(Self::error)?
            .detach()
            .map_err(Self::error)?;
        Ok(())
    }
    fn discard_pair(volumes: BootstrapVolumes<'_, '_, '_, Unmounted>) -> Result<(), BuildError> {
        let (tool, output) = volumes.into_parts();
        let tool = Self::discard(tool);
        let output = Self::discard(output);
        match (tool, output) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(tool), Err(output)) => Err(Self::error(format!(
                "tool cleanup: {tool}; output cleanup: {output}"
            ))),
        }
    }
    fn reject(failure: StageFailure<'_, BootstrapVolumes<'_, '_, '_, Mounted>>) -> BuildError {
        let primary = failure.to_string();
        let cleanup = match failure {
            StageFailure::Before { storage, .. } => Self::discard_pair(storage),
            StageFailure::Preparation { failure, .. } => failure
                .rollback()
                .map_err(Self::error)
                .and_then(|done| Self::discard_pair(done.into_parts().1)),
            StageFailure::Execution { failure, .. } => failure
                .retry_cleanup()
                .into_rejected_storage()
                .map_err(Self::error)
                .and_then(|(_, storage)| Self::discard_pair(storage)),
            StageFailure::NetworkSetup { failure, .. } => failure
                .retry_cleanup()
                .map_err(Self::error)
                .and_then(|failure| {
                    failure
                        .retry_cleanup()
                        .into_rejected_storage()
                        .map_err(Self::error)
                        .and_then(|(_, storage)| Self::discard_pair(storage))
                }),
            StageFailure::NetworkExecution { failure, .. } => failure
                .retry_cleanup()
                .into_rejected_storage()
                .map_err(Self::error)
                .and_then(|(_, storage)| Self::discard_pair(storage)),
            StageFailure::Copy { failure, .. } => failure
                .retry_cleanup()
                .into_rejected_storage()
                .map_err(Self::error)
                .and_then(|(_, storage, _)| Self::discard_pair(storage)),
        };
        match cleanup {
            Ok(()) => Self::error(primary),
            Err(error) => Self::error(format!("{primary}; cleanup: {error}")),
        }
    }
}
