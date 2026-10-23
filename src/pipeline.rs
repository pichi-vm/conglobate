// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Recipe orchestration. Unsupported execution paths fail before publication.
mod bootstrap;
mod carapace;
mod firmware;
use crate::{
    cache::{Cache, CacheError},
    input::{CapturedBuild, CheckedImage, InputError, ResolvedBuild},
    inventory::{Inventory, InventoryError},
    output::{AssembledImage, AssemblyFailure, PublicationFailure},
    request::BuildRequest,
    storage::Workspace,
};

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
    #[error(transparent)]
    Transform(#[from] crate::transform::TransformError),
    #[error("build path is not implemented yet: {0}")]
    Unsupported(&'static str),
    #[error("build execution: {0}")]
    Execution(String),
    #[error("conglobate requires root")]
    RootRequired,
}
impl From<crate::storage::RuntimeScopeFailure<'_, BuildError>> for BuildError {
    fn from(failure: crate::storage::RuntimeScopeFailure<'_, BuildError>) -> Self {
        match failure {
            crate::storage::RuntimeScopeFailure::Operation(error) => error,
            failure => Self::Execution(failure.to_string()),
        }
    }
}
#[derive(Debug)]
pub struct Pipeline {
    request: BuildRequest,
}
impl Pipeline {
    /// Retain a parsed request without resolving inputs or acquiring resources.
    pub fn new(request: BuildRequest) -> Self {
        Self { request }
    }
    /// Resolve local inputs, execute the recipe and publish its verified output.
    /// The caller supplies the dedicated disposable-VM environment; this method
    /// does not manage VM shutdown or turn the calling process into a sandbox.
    ///
    /// # Errors
    /// Rejects non-root execution, invalid/unavailable inputs, unsupported work,
    /// failed stage execution, cleanup, assembly, inventory or publication.
    /// Ordinary errors do not authorize another build in the same VM. Some
    /// failures retain unfinished kernel resources until environment disposal;
    /// returning an error is not a successful teardown receipt.
    pub fn run(self) -> Result<(), BuildError> {
        if !rustix::process::geteuid().is_root() {
            return Err(BuildError::RootRequired);
        }
        let build =
            CapturedBuild::try_from(&self.request)?.resolve(Cache::open(self.request.cache())?)?;
        let workspace = Workspace::create()?;
        let context = workspace
            .materialize(build.captured().context())?
            .read_only()?;
        let output = if build.captured().plan().recipe().bootstrap().is_some() {
            bootstrap::BootstrapBuild::new(&build)?.execute(&self.request, &workspace)?
        } else if build.captured().plan().recipe().firmware().is_some() {
            firmware::FirmwareBuild::new(&build)?.execute(&self.request, &workspace)?
        } else if build
            .captured()
            .plan()
            .recipe()
            .carapace()
            .is_some_and(|stage| !stage.directives().is_empty())
        {
            carapace::CarapaceBuild::new(&build)?.execute(&self.request, &workspace)?
        } else {
            InheritedBuild::new(&build)?.assemble(&self.request)?
        };
        // No artifact can be published if private input cleanup fails.
        context.close()?;
        drop(workspace);
        output.publish()?;
        Ok(())
    }
}
/// Selected inherited-image path with no command/COPY/firmware work.
/// DTB transformations still execute before inventory and publication.
#[derive(Debug)]
pub(crate) struct InheritedBuild<'a> {
    build: &'a ResolvedBuild,
    base: &'a CheckedImage,
}
impl<'a> InheritedBuild<'a> {
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
        Ok(Self { build, base })
    }
    pub(crate) fn build(&self) -> &ResolvedBuild {
        self.build
    }
    pub(crate) fn assemble(&self, request: &BuildRequest) -> Result<CompletedImage, BuildError> {
        let source = self
            .base
            .image()
            .manifest()
            .layers
            .iter()
            .find_map(|layer| {
                if let pichi_artifact::Layer::Dtb(descriptor) = layer {
                    Some(descriptor)
                } else {
                    None
                }
            })
            .map(|d| {
                let digest = d
                    .digest
                    .parse()
                    .map_err(|_| CacheError::Invalid("invalid DTB digest"))?;
                self.build.cache().metadata(&digest, Some(d.size))
            })
            .transpose()?;
        let root = self.base.chain().map(|chain| hex::encode(chain.root()));
        let transformed = self
            .build
            .captured()
            .plan()
            .recipe()
            .transform()
            .apply(source.as_ref().map(|s| s.bytes()), root.as_deref())?;
        let image = AssembledImage::transformed(
            self.build.cache(),
            self.base.image(),
            request.output(),
            &transformed,
        )?;
        let inventory = Inventory::inherited(request, self, &image, &transformed)?;
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
