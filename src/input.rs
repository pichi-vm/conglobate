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
pub(crate) enum InputError {
    #[error("executing stages require --uid-map and --gid-map")]
    IdMappingsRequired,
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
    #[error("network-enabled commands require --network-namespace")]
    NetworkNamespaceRequired,
    #[error("network namespace input: {0}")]
    NetworkNamespace(#[source] std::io::Error),
    #[error("resolver input: {0}")]
    Resolver(std::io::Error),
}

/// Selected context-relative name and digest of the exact resolved file bytes.
#[derive(Debug)]
pub(crate) struct SelectedDocument {
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
    pub(crate) fn path(&self) -> &ContextPath {
        &self.path
    }
    pub(crate) fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

/// Owns all committed bytes used to derive the plan. This is not an executable
/// build: cache verification and execution preparation are separate gates.
#[derive(Debug)]
pub(crate) struct CapturedBuild {
    id_mappings: Option<crate::isolation::IdMappings>,
    context: CapturedContext,
    recipe: SelectedDocument,
    lock: SelectedDocument,
    plan: BuildPlan,
    networking: Option<crate::network::NetworkInput>,
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
        let networking = if plan.network_steps().is_empty() {
            None
        } else {
            let path = request.resolv_conf().ok_or(InputError::ResolverRequired)?;
            let resolver =
                crate::network::ResolverInput::capture(path).map_err(InputError::Resolver)?;
            let namespace = request
                .network_namespace()
                .ok_or(InputError::NetworkNamespaceRequired)?;
            let namespace = crate::network::NetworkNamespace::open(namespace)
                .map_err(InputError::NetworkNamespace)?;
            Some(crate::network::NetworkInput::new(namespace, resolver))
        };
        let id_mappings = if plan.execution_required() {
            Some(
                request
                    .id_mappings()
                    .ok_or(InputError::IdMappingsRequired)?,
            )
        } else {
            None
        };
        let recipe = SelectedDocument::new(request.recipe().clone(), &recipe_file);
        let lock = SelectedDocument::new(request.lock().clone(), &lock_file);
        Ok(Self {
            id_mappings,
            context,
            recipe,
            lock,
            plan,
            networking,
        })
    }
}
impl CapturedBuild {
    pub(crate) fn id_mappings(&self) -> Option<crate::isolation::IdMappings> {
        self.id_mappings
    }
    pub(crate) fn execution_mappings(&self) -> Result<crate::isolation::IdMappings, InputError> {
        self.id_mappings.ok_or(InputError::IdMappingsRequired)
    }

    pub(crate) fn context(&self) -> &CapturedContext {
        &self.context
    }
    pub(crate) fn recipe_document(&self) -> &SelectedDocument {
        &self.recipe
    }
    pub(crate) fn lock_document(&self) -> &SelectedDocument {
        &self.lock
    }
    pub(crate) fn networking(&self) -> Option<&crate::network::NetworkInput> {
        self.networking.as_ref()
    }
    pub(crate) fn plan(&self) -> &BuildPlan {
        &self.plan
    }
}

/// One locked image and its verified chain (if present). Receipts describe reads,
/// not a promise that a subsequent cache read will return unchanged bytes.
#[derive(Debug)]
pub(crate) struct CheckedImage {
    image: crate::cache::ResolvedImage,
    chain: Option<crate::cache::VerifiedChain>,
}
impl CheckedImage {
    /// Capture the selected DTB with a fresh digest/size check. Earlier input
    /// receipts do not authorize trusting mutable cache bytes on this read.
    pub(crate) fn inherited_dtb(
        &self,
        cache: &crate::cache::Cache,
    ) -> Result<Option<crate::cache::VerifiedMetadata>, crate::cache::CacheError> {
        self.image
            .manifest()
            .layers
            .iter()
            .find_map(|layer| match layer {
                crate::artifact::Layer::Dtb(descriptor) => Some(descriptor),
                _ => None,
            })
            .map(|descriptor| {
                let digest = descriptor
                    .digest
                    .parse()
                    .map_err(|_| crate::cache::CacheError::Invalid("invalid DTB digest"))?;
                cache.metadata(&digest, Some(descriptor.size))
            })
            .transpose()
    }
    pub(crate) fn image(&self) -> &crate::cache::ResolvedImage {
        &self.image
    }
    pub(crate) fn chain(&self) -> Option<&crate::cache::VerifiedChain> {
        self.chain.as_ref()
    }
}
#[derive(Debug)]
pub(crate) struct ResolvedBuild {
    captured: CapturedBuild,
    cache: crate::cache::Cache,
    images: std::collections::BTreeMap<crate::image::ImageReference, CheckedImage>,
}
impl ResolvedBuild {
    pub(crate) fn captured(&self) -> &CapturedBuild {
        &self.captured
    }
    pub(crate) fn cache(&self) -> &crate::cache::Cache {
        &self.cache
    }
    pub(crate) fn images(
        &self,
    ) -> &std::collections::BTreeMap<crate::image::ImageReference, CheckedImage> {
        &self.images
    }
}
impl CapturedBuild {
    /// Consume the captured planning phase after resolving all locked inputs.
    /// No privileged setup or output publication occurs here.
    pub(crate) fn resolve(self, cache: crate::cache::Cache) -> Result<ResolvedBuild, InputError> {
        let mut images = std::collections::BTreeMap::new();
        for input in self.plan.inputs() {
            let image = cache.resolve(input.index())?;
            let chain = cache.verify_content(&image)?;
            if input.execution_required() && chain.is_none() {
                return Err(InputError::MissingExecutionCarapace(
                    input.reference().as_ref().to_owned(),
                ));
            }
            images.insert(input.reference().clone(), CheckedImage { image, chain });
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
                        .any(|layer| matches!(layer, crate::artifact::Layer::Dtb(_)))
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
