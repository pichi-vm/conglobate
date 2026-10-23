// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Recipe orchestration. Unsupported execution paths fail before publication.
use crate::{
    cache::{Cache, CacheError},
    input::{CapturedBuild, CheckedImage, InputError, ResolvedBuild},
    inventory::{Inventory, InventoryError},
    output::{AssembledImage, AssemblyFailure, PublicationFailure},
    recipe::TransformSelection,
    request::BuildRequest,
    storage::Workspace,
};

pub(crate) use crate::recipe::DEFAULT_TRANSFORM;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error(transparent)]
    Input(#[from] InputError),
    #[error(transparent)]
    Cache(#[from] CacheError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Materialize(#[from] crate::context::ContextMaterializeFailure),
    #[error(transparent)]
    ReadOnly(#[from] crate::context::ContextMountFailure),
    #[error(transparent)]
    Cleanup(#[from] crate::context::ContextCleanupFailure),
    #[error(transparent)]
    Assembly(#[from] AssemblyFailure),
    #[error(transparent)]
    Inventory(#[from] InventoryError),
    #[error(transparent)]
    Publication(#[from] PublicationFailure),
    #[error("build path is not implemented yet: {0}")]
    Unsupported(&'static str),
    #[error("conglobate requires root")]
    RootRequired,
}
#[derive(Debug)]
pub struct Pipeline {
    request: BuildRequest,
}
impl Pipeline {
    pub fn new(request: BuildRequest) -> Self {
        Self { request }
    }
    pub fn run(self) -> Result<(), BuildError> {
        if !rustix::process::geteuid().is_root() {
            return Err(BuildError::RootRequired);
        }
        let build =
            CapturedBuild::try_from(&self.request)?.resolve(Cache::open(self.request.cache())?)?;
        let selected = PassThrough::new(&build)?;
        let workspace = Workspace::create()?;
        let context = workspace
            .materialize(build.captured().context())?
            .read_only()?;
        let output = selected.assemble(&self.request)?;
        // No artifact can be published if private input cleanup fails.
        context.close()?;
        drop(workspace);
        output.publish()?;
        Ok(())
    }
}
/// Private proof that this recipe has no command/COPY/firmware work and that
/// its effective transformation is a no-op. This is not a fallback for stages.
#[derive(Debug)]
pub(crate) struct PassThrough<'a> {
    build: &'a ResolvedBuild,
    base: &'a CheckedImage,
    authored: bool,
}
impl<'a> PassThrough<'a> {
    pub(crate) fn new(build: &'a ResolvedBuild) -> Result<Self, BuildError> {
        let recipe = build.captured().plan().recipe();
        if recipe.bootstrap().is_some() {
            return Err(BuildError::Unsupported("bootstrap coordinator"));
        }
        if recipe.firmware().is_some() {
            return Err(BuildError::Unsupported("firmware coordinator"));
        }
        let stage = recipe
            .carapace()
            .ok_or(BuildError::Unsupported("missing output base"))?;
        if !stage.directives().is_empty() {
            return Err(BuildError::Unsupported("carapace directives"));
        }
        let base = stage
            .from()
            .and_then(|name| build.images().get(name))
            .ok_or(BuildError::Unsupported("missing resolved output base"))?;
        let dtb = base
            .image()
            .manifest()
            .layers
            .iter()
            .any(|l| matches!(l, pichi_artifact::Layer::Dtb(_)));
        let authored = match recipe.transform() {
            TransformSelection::Authored(ops) if ops.is_empty() && dtb => true,
            TransformSelection::Builtin if !dtb => false,
            _ => return Err(BuildError::Unsupported("DTB transformations")),
        };
        Ok(Self {
            build,
            base,
            authored,
        })
    }
    pub(crate) fn build(&self) -> &ResolvedBuild {
        self.build
    }
    pub(crate) fn authored(&self) -> bool {
        self.authored
    }
    pub(crate) fn assemble(&self, request: &BuildRequest) -> Result<CompletedImage, BuildError> {
        let image =
            AssembledImage::inherit(self.build.cache(), self.base.image(), request.output())?;
        let inventory = Inventory::pass_through(request, self, &image)?;
        inventory.check_output(&image)?;
        Ok(CompletedImage { image, inventory })
    }
}
/// Created only after the selected recipe path completed and inventory binds
/// its finalized output. Neither decoded inventory nor raw storage can create it.
#[derive(Debug)]
pub(crate) struct CompletedImage {
    image: AssembledImage,
    inventory: Inventory,
}
impl CompletedImage {
    pub(crate) fn publish(self) -> Result<(), BuildError> {
        self.image.publish_inventory(&self.inventory)?;
        Ok(())
    }
}
