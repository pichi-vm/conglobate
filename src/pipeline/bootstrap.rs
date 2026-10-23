// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{BuildError, CompletedImage, rejection::DiscardStorage};
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
    step::{CommandSandbox, SetupBudget},
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
        let manifest = request.formatter_runtime().ok_or_else(|| {
            BuildError::InvalidExecution("bootstrap requires --formatter-runtime")
        })?;
        let captured = FormatterManifest::capture(manifest)?;
        Runtime::scoped(workspace, Path::new("/run"), |runtime| {
            match captured.stage(workspace) {
                Ok(formatter) => self.produce(request, workspace, runtime, formatter),
                Err(failure) => {
                    let (primary, cleanup) = failure.cleanup_with_source();
                    Err(BuildError::FormatterStaging(primary)
                        .with_cleanup(cleanup.map_err(BuildError::from)))
                }
            }
        })
        .map_err(BuildError::from)
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
                    .map_err(BuildError::from)?,
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
                    active.close().map_err(BuildError::from)?;
                }
                let tools = self.build.images().get(reference).ok_or_else(|| {
                    BuildError::InvalidExecution("missing resolved firmware tools")
                })?;
                tools
                    .chain()
                    .ok_or_else(|| {
                        BuildError::InvalidExecution("firmware tools require a root volume")
                    })?
                    .assemble(self.build.cache(), workspace)
                    .map_err(BuildError::from)?
            } else {
                match active {
                    Some(active) => active,
                    None => crate::storage::ReadOnlyChain::from_root(Arc::clone(&produced[0]))
                        .map_err(BuildError::from)?,
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
                active.close().map_err(BuildError::from)?;
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
