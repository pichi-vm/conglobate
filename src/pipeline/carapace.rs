// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{BuildError, CompletedImage};
use crate::{
    cache::CacheError,
    execution::HelperProgram,
    input::{CheckedImage, ResolvedBuild},
    inventory::Inventory,
    isolation::IdRange,
    output::AssembledImage,
    process::ChildReaper,
    recipe::Directive,
    request::BuildRequest,
    step::{CommandSandbox, StageFailure},
    storage::{Created, Mapping, Mounted, Runtime, SealedScute, Unmounted, Volume, Workspace},
};
use std::{num::NonZeroU64, path::Path, sync::Arc};

/// Validated execution selection; output ancestry is distinct from tool inputs.
#[derive(Debug)]
pub(super) struct CarapaceBuild<'a> {
    build: &'a ResolvedBuild,
    base: &'a CheckedImage,
    directives: &'a [Directive],
}
impl<'a> CarapaceBuild<'a> {
    pub(super) fn new(build: &'a ResolvedBuild) -> Result<Self, BuildError> {
        let recipe = build.captured().plan().recipe();
        if recipe.bootstrap().is_some() {
            return Err(BuildError::Unsupported("bootstrap coordinator"));
        }
        if recipe.firmware().is_some() {
            return Err(BuildError::Unsupported("firmware coordinator"));
        }
        let stage = recipe
            .carapace()
            .ok_or(BuildError::Unsupported("missing carapace stage"))?;
        if stage
            .directives()
            .iter()
            .any(|d| matches!(d, Directive::Run(run) if run.network()))
        {
            return Err(BuildError::Unsupported("enabled networking"));
        }
        let base = stage
            .from()
            .and_then(|name| build.images().get(name))
            .ok_or(BuildError::Unsupported("missing resolved base"))?;
        if base.chain().is_none() {
            return Err(BuildError::Cache(CacheError::Invalid(
                "carapace execution requires a root volume",
            )));
        }
        Ok(Self {
            build,
            base,
            directives: stage.directives(),
        })
    }
    pub(super) fn execute(
        &self,
        request: &BuildRequest,
        workspace: &Workspace,
    ) -> Result<CompletedImage, BuildError> {
        let runtime = Runtime::create(workspace, Path::new("/run")).map_err(Self::error)?;
        let result = self.derive(request, workspace, &runtime);
        let cleanup = runtime.close().map_err(Self::error);
        match (result, cleanup) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => {
                Err(Self::error(format!("{error}; runtime cleanup: {cleanup}")))
            }
        }
    }
    fn derive(
        &self,
        request: &BuildRequest,
        workspace: &Workspace,
        runtime: &Runtime<'_>,
    ) -> Result<CompletedImage, BuildError> {
        let chain = self.base.chain().expect("validated root volume");
        let mut active = chain
            .assemble(self.build.cache(), workspace)
            .map_err(Self::error)?;
        let paths = runtime.paths()?;
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
        );
        let mut reaper = ChildReaper::install()?;
        let mut produced = Vec::<Arc<SealedScute<'_>>>::new();
        for directive in self.directives {
            let scratch = workspace.snapshot_store()?;
            let snapshot = Mapping::<Created>::create()?
                .snapshot(
                    &active,
                    scratch.attach().map_err(Self::error)?,
                    NonZeroU64::new(crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512)
                        .unwrap(),
                )
                .map_err(Self::error)?;
            let volume = Volume::<Unmounted>::in_init_namespace(snapshot, runtime.recovery())
                .map_err(Self::error)?;
            let completed = sandbox
                .directives::<Volume<'_, '_, '_, Mounted>>(
                    volume,
                    reaper,
                    self.build.captured().context(),
                    std::slice::from_ref(directive),
                    None,
                )
                .map_err(|failure| Self::reject(*failure))?;
            let (volume, returned_reaper) = completed.into_parts();
            reaper = returned_reaper;
            let scute = Arc::new(
                volume
                    .into_snapshot()
                    .close()
                    .map_err(Self::error)?
                    .detach()
                    .map_err(Self::error)?
                    .seal_snapshot(*active.root())
                    .map_err(Self::error)?,
            );
            active = active.extend(Arc::clone(&scute)).map_err(Self::error)?;
            produced.push(scute);
        }
        active.close().map_err(Self::error)?;
        let source = self
            .base
            .image()
            .manifest()
            .layers
            .iter()
            .find_map(|layer| match layer {
                pichi_artifact::Layer::Dtb(descriptor) => Some(descriptor),
                _ => None,
            })
            .map(|d| {
                let digest = d
                    .digest
                    .parse()
                    .map_err(|_| CacheError::Invalid("invalid DTB digest"))?;
                self.build.cache().metadata(&digest, Some(d.size))
            })
            .transpose()?;
        let root = hex::encode(produced.last().expect("nonempty directives").root());
        let transformed = self
            .build
            .captured()
            .plan()
            .recipe()
            .transform()
            .apply(source.as_ref().map(|s| s.bytes()), Some(&root))?;
        let image = AssembledImage::derived(
            self.build.cache(),
            self.base.image(),
            &produced,
            request.output(),
            &transformed,
        )?;
        let inventory =
            Inventory::from_build(request, self.build, &image, &transformed, &produced)?;
        inventory.check_output(&image)?;
        Ok(CompletedImage { image, inventory })
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
    pub(super) fn reject(failure: StageFailure<'_, Volume<'_, '_, '_, Mounted>>) -> BuildError {
        let primary = failure.to_string();
        let cleanup = match failure {
            StageFailure::Before { storage, .. } => Self::discard(storage),
            StageFailure::Preparation { failure, .. } => failure
                .rollback()
                .map_err(Self::error)
                .and_then(|done| Self::discard(done.into_parts().1)),
            StageFailure::Execution { failure, .. } => match failure.retry_cleanup() {
                Ok(done) => Self::discard(done.into_storage()),
                Err(failure) => failure
                    .into_rejected_storage()
                    .map_err(Self::error)
                    .and_then(|(_, storage)| Self::discard(storage)),
            },
            StageFailure::Copy { failure, .. } => failure
                .retry_cleanup()
                .into_rejected_storage()
                .map_err(Self::error)
                .and_then(|(_, storage, _)| Self::discard(storage)),
        };
        match cleanup {
            Ok(()) => Self::error(primary),
            Err(error) => Self::error(format!("{primary}; cleanup: {error}")),
        }
    }
}
