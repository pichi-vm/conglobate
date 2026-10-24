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
    step::CommandSandbox,
    storage::{Created, Mapping, Mounted, Runtime, SealedScute, Unmounted, Volume, Workspace},
};
use std::{num::NonZeroU64, sync::Arc};

/// Validated execution selection; output ancestry is distinct from tool inputs.
#[derive(Debug)]
pub(super) struct CarapaceBuild<'a> {
    build: &'a ResolvedBuild,
    base: &'a CheckedImage,
    directives: &'a [Directive],
}
/// Successful retained layers and their live read-only chain, ready for the next
/// stage. The reaper transfers with the build instead of being reinstalled.
#[derive(Debug)]
pub(super) struct DerivedCarapace<'s, 'w> {
    pub(super) active: crate::storage::ReadOnlyChain<'s, 'w>,
    pub(super) scutes: Vec<Arc<SealedScute<'w>>>,
    pub(super) reaper: ChildReaper,
}
impl<'a> CarapaceBuild<'a> {
    pub(super) fn new(build: &'a ResolvedBuild) -> Result<Self, BuildError> {
        let recipe = build.captured().plan().recipe();
        if recipe.bootstrap().is_some() {
            return Err(BuildError::Unsupported("bootstrap coordinator"));
        }
        let stage = recipe
            .carapace()
            .ok_or(BuildError::Unsupported("missing carapace stage"))?;
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

    pub(super) fn run_layers<'w>(
        &self,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
    ) -> Result<DerivedCarapace<'a, 'w>, BuildError> {
        let chain = self.base.chain().expect("validated root volume");
        let active = chain
            .assemble(self.build.cache(), workspace)
            .map_err(BuildError::from)?;
        DerivedCarapace {
            active,
            scutes: Vec::new(),
            reaper: ChildReaper::install()?,
        }
        .derive(self.build, workspace, runtime, self.directives)
    }
    pub(super) fn execute(
        &self,
        request: &BuildRequest,
        workspace: &Workspace,
        runtime: &Runtime<'_>,
    ) -> Result<CompletedImage, BuildError> {
        let DerivedCarapace {
            active,
            scutes: produced,
            reaper: _reaper,
        } = self.run_layers(workspace, runtime)?;
        active.close().map_err(BuildError::from)?;
        let source = self.base.inherited_dtb(self.build.cache())?;
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
}

impl<'s, 'w> DerivedCarapace<'s, 'w> {
    pub(super) fn derive(
        self,
        build: &ResolvedBuild,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        directives: &[Directive],
    ) -> Result<Self, BuildError> {
        let Self {
            mut active,
            scutes: mut produced,
            mut reaper,
        } = self;
        let paths = runtime.paths()?;
        let sandbox = CommandSandbox::new(
            HelperProgram::current()?,
            paths.sandbox(),
            IdRange::new(100000, 65536)?,
            IdRange::new(100000, 65536)?,
        );
        for (index, directive) in directives.iter().enumerate() {
            let scratch = workspace.snapshot_store()?;
            let snapshot = Mapping::<Created>::create()?
                .snapshot(
                    &active,
                    scratch.attach().map_err(BuildError::from)?,
                    NonZeroU64::new(crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES / 512)
                        .unwrap(),
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
                    std::slice::from_ref(directive),
                    None,
                    networking.as_ref(),
                )
                .map_err(|failure| (*failure).reject_at(index))?;
            let (volume, returned_reaper) = completed.into_parts();
            reaper = returned_reaper;
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
            active = active
                .extend(Arc::clone(&scute))
                .map_err(BuildError::from)?;
            produced.push(scute);
        }
        Ok(DerivedCarapace {
            active,
            scutes: produced,
            reaper,
        })
    }
}
