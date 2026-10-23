// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Recipe planning from one immutable committed context. Image verification follows.
use crate::{
    context::{CapturedContext, ContextError, ContextFile},
    document::YamlDocument,
    lock::{ImageLock, LockError},
    path::ContextPath,
    plan::BuildPlan,
    recipe::{Recipe, RecipeError},
    request::BuildRequest,
};
use sha2::{Digest, Sha256};

#[derive(Debug, thiserror::Error)]
pub enum InputError {
    #[error("an authored transform requires an inherited or generated DTB")]
    MissingTransformDtb,
    #[error(transparent)]
    Cache(#[from] crate::cache::CacheError),
    #[error("execution input {0} has no carapace")]
    MissingExecutionCarapace(String),
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error(transparent)]
    Recipe(#[from] RecipeError),
    #[error(transparent)]
    Lock(#[from] LockError),
    #[error("network-enabled commands require --resolv-conf")]
    ResolverRequired,
    #[error("resolver input: {0}")]
    Resolver(std::io::Error),
}

/// Selected context-relative name and digest of the exact resolved file bytes.
#[derive(Debug)]
pub struct SelectedDocument {
    path: ContextPath,
    sha256: [u8; 32],
}
impl SelectedDocument {
    fn new(path: ContextPath, file: &ContextFile<'_>) -> Self {
        Self {
            path,
            sha256: Sha256::digest(file.bytes()).into(),
        }
    }
    pub fn path(&self) -> &ContextPath {
        &self.path
    }
    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

/// Owns all committed bytes used to derive the plan. This is not an executable
/// build: cache verification and private materialization are separate gates.
#[derive(Debug)]
pub struct CapturedBuild {
    context: CapturedContext,
    recipe: SelectedDocument,
    lock: SelectedDocument,
    plan: BuildPlan,
    resolver: Option<crate::network::ResolverInput>,
}
impl TryFrom<&BuildRequest> for CapturedBuild {
    type Error = InputError;
    fn try_from(request: &BuildRequest) -> Result<Self, Self::Error> {
        let context =
            CapturedContext::capture(request.repository(), request.commit(), request.context())?;
        let recipe_file = context.read(request.recipe())?;
        let lock_file = context.read(request.lock())?;
        let recipe = Recipe::try_from(YamlDocument::from(recipe_file.text()?))?;
        let lock = ImageLock::try_from(YamlDocument::from(lock_file.text()?))?;
        let plan = BuildPlan::new(recipe, &lock)?;
        let resolver = if plan.network_steps().is_empty() {
            None
        } else {
            let path = request.resolv_conf().ok_or(InputError::ResolverRequired)?;
            Some(crate::network::ResolverInput::capture(path).map_err(InputError::Resolver)?)
        };
        let recipe = SelectedDocument::new(request.recipe().clone(), &recipe_file);
        let lock = SelectedDocument::new(request.lock().clone(), &lock_file);
        Ok(Self {
            context,
            recipe,
            lock,
            plan,
            resolver,
        })
    }
}
impl CapturedBuild {
    pub fn context(&self) -> &CapturedContext {
        &self.context
    }
    pub fn recipe_document(&self) -> &SelectedDocument {
        &self.recipe
    }
    pub fn lock_document(&self) -> &SelectedDocument {
        &self.lock
    }
    pub fn resolver(&self) -> Option<&crate::network::ResolverInput> {
        self.resolver.as_ref()
    }
    pub fn plan(&self) -> &BuildPlan {
        &self.plan
    }
}

/// One locked image and its verified chain (if present). Receipts describe reads,
/// not a promise that a subsequent cache read will return unchanged bytes.
#[derive(Debug)]
pub struct CheckedImage {
    image: crate::cache::ResolvedImage,
    chain: Option<crate::cache::VerifiedChain>,
    receipts: Vec<crate::cache::BlobReceipt>,
}
impl CheckedImage {
    pub fn image(&self) -> &crate::cache::ResolvedImage {
        &self.image
    }
    pub fn chain(&self) -> Option<&crate::cache::VerifiedChain> {
        self.chain.as_ref()
    }
    pub fn receipts(&self) -> &[crate::cache::BlobReceipt] {
        &self.receipts
    }
}
#[derive(Debug)]
pub struct ResolvedBuild {
    captured: CapturedBuild,
    cache: crate::cache::Cache,
    images: std::collections::BTreeMap<crate::image::ImageReference, CheckedImage>,
}
impl ResolvedBuild {
    pub fn captured(&self) -> &CapturedBuild {
        &self.captured
    }
    pub fn cache(&self) -> &crate::cache::Cache {
        &self.cache
    }
    pub fn images(
        &self,
    ) -> &std::collections::BTreeMap<crate::image::ImageReference, CheckedImage> {
        &self.images
    }
}
impl CapturedBuild {
    /// Consume the captured planning phase after resolving all locked inputs.
    /// No privileged setup or output publication occurs here.
    pub fn resolve(self, cache: crate::cache::Cache) -> Result<ResolvedBuild, InputError> {
        let mut images = std::collections::BTreeMap::new();
        for input in self.plan.inputs() {
            let image = cache.resolve(input.index())?;
            let (receipts, chain) = cache.verify_content(&image)?;
            if input.execution_required() && chain.is_none() {
                return Err(InputError::MissingExecutionCarapace(
                    input.reference().as_ref().to_owned(),
                ));
            }
            images.insert(
                input.reference().clone(),
                CheckedImage {
                    image,
                    chain,
                    receipts,
                },
            );
        }
        let recipe = self.plan.recipe();
        if recipe.firmware().is_none()
            && matches!(
                recipe.transform(),
                crate::recipe::TransformSelection::Authored(_)
            )
        {
            // Only the selected output base propagates components. Bootstrap
            // tools (and alternate firmware tools) are never output ancestry.
            let inherited_dtb = recipe
                .carapace()
                .and_then(|stage| stage.from())
                .and_then(|reference| images.get(reference))
                .is_some_and(|image| {
                    image
                        .image
                        .manifest()
                        .layers
                        .iter()
                        .any(|layer| matches!(layer, pichi_artifact::Layer::Dtb(_)))
                });
            if !inherited_dtb {
                return Err(InputError::MissingTransformDtb);
            }
        }
        Ok(ResolvedBuild {
            captured: self,
            cache,
            images,
        })
    }
}
