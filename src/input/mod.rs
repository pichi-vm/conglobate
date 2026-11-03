// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Build planning from one immutable committed context. Image verification follows.
mod availability;
use crate::{
    context::{CapturedContext, ContextError},
    document::YamlDocument,
    graph::BuildGraph,
    lock::{ImageLock, LockError},
    path::ContextPath,
    planner::{Plan, PlanError},
    request::BuildRequest,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum InputError {
    #[error(transparent)]
    Identity(#[from] crate::image::IdentityError),
    #[error("executing stages require --uid-map and --gid-map")]
    IdMappingsRequired,
    #[error(transparent)]
    Cache(#[from] crate::cache::CacheError),
    #[error("{0}")]
    Components(String),
    #[error("execution input {0} has no carapace")]
    MissingExecutionCarapace(String),
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error(transparent)]
    Plan(#[from] PlanError),
    #[error(transparent)]
    Selection(#[from] crate::copy::SelectionError),
    #[error(transparent)]
    Lock(#[from] LockError),
    #[error("resolver input /etc/resolv.conf: {0}")]
    Resolver(std::io::Error),
}

/// Selected context-relative name of a resolved document.
#[derive(Debug)]
pub(crate) struct SelectedDocument {
    path: ContextPath,
}
impl SelectedDocument {
    fn new(path: ContextPath) -> Self {
        Self { path }
    }
    pub(crate) fn path(&self) -> &ContextPath {
        &self.path
    }
}

/// Owns all committed bytes used to derive the graph. This is not an executable
/// build: cache verification and execution preparation are separate gates.
#[derive(Debug)]
pub(crate) struct CapturedBuild {
    id_mappings: Option<crate::isolation::IdMappings>,
    context: CapturedContext,
    plan: SelectedDocument,
    lock: SelectedDocument,
    graph: BuildGraph,
    networking: Option<crate::network::ResolverInput>,
}
impl TryFrom<&BuildRequest> for CapturedBuild {
    type Error = InputError;
    fn try_from(request: &BuildRequest) -> Result<Self, Self::Error> {
        let mut context =
            CapturedContext::capture(request.repository(), request.commit(), request.context())?;
        let plan_file = context.read(request.plan())?;
        let lock_file = context.read(request.lock())?;
        let plan = Plan::try_from(YamlDocument::from(plan_file.text()?))?;
        let lock = ImageLock::try_from(YamlDocument::from(lock_file.text()?))?;
        let graph = BuildGraph::new(plan, &lock)?;
        for name in &graph.plan().order {
            for directive in &graph.plan().stages[name].derive {
                if let crate::planner::Directive::Copy(copy) = directive
                    && copy.from().is_none()
                {
                    crate::copy::CopySources::prepare(&mut context, copy.sources())?;
                }
            }
        }

        let networking = if graph.network_steps().is_empty() {
            None
        } else {
            Some(
                crate::network::ResolverInput::capture(std::path::Path::new("/etc/resolv.conf"))
                    .map_err(InputError::Resolver)?,
            )
        };
        let id_mappings = if graph.plan().commands_required() {
            Some(
                request
                    .id_mappings()
                    .ok_or(InputError::IdMappingsRequired)?,
            )
        } else {
            None
        };
        let plan = SelectedDocument::new(request.plan().clone());
        let lock = SelectedDocument::new(request.lock().clone());
        Ok(Self {
            id_mappings,
            context,
            plan,
            lock,
            graph,
            networking,
        })
    }
}
impl CapturedBuild {
    pub(crate) fn execution_mappings(&self) -> Result<crate::isolation::IdMappings, InputError> {
        self.id_mappings.ok_or(InputError::IdMappingsRequired)
    }

    pub(crate) fn context(&self) -> &CapturedContext {
        &self.context
    }
    pub(crate) fn plan_document(&self) -> &SelectedDocument {
        &self.plan
    }
    pub(crate) fn lock_document(&self) -> &SelectedDocument {
        &self.lock
    }
    pub(crate) fn networking(&self) -> Option<&crate::network::ResolverInput> {
        self.networking.as_ref()
    }
    pub(crate) fn graph(&self) -> &BuildGraph {
        &self.graph
    }
}

/// One locked image with authenticated metadata. Carapace verifies filesystem
/// reads against its root when execution activates the prepared input files.
#[derive(Debug)]
pub(crate) struct CheckedImage {
    image: crate::cache::ResolvedImage,
}
impl CheckedImage {
    pub(crate) fn verify(
        cache: &crate::cache::Cache,
        image: crate::cache::ResolvedImage,
    ) -> Result<Self, crate::cache::CacheError> {
        cache.verify_metadata(&image)?;
        Ok(Self { image })
    }

    /// Capture selected native config with a fresh digest/size check.
    pub(crate) fn bootconfig(
        &self,
        cache: &crate::cache::Cache,
    ) -> Result<Option<crate::cache::VerifiedMetadata>, crate::cache::CacheError> {
        self.image
            .manifest()
            .layers
            .iter()
            .find(|layer| {
                layer.media_type().as_ref() == crate::artifact::MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD
            })
            .map(|descriptor| cache.metadata(descriptor.digest(), Some(descriptor.size())))
            .transpose()
    }
    pub(crate) fn image(&self) -> &crate::cache::ResolvedImage {
        &self.image
    }
    pub(crate) fn root(&self) -> Option<[u8; 32]> {
        self.image.root()
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
        for input in self.graph.inputs() {
            let image = cache.resolve(input.index())?;
            let image = CheckedImage::verify(&cache, image)?;
            if input.root_required() && image.root().is_none() {
                return Err(InputError::MissingExecutionCarapace(
                    input.reference().as_ref().to_owned(),
                ));
            }
            images.insert(input.reference().clone(), image);
        }
        availability::Availability::validate(self.graph.plan(), &images)?;
        Ok(ResolvedBuild {
            captured: self,
            cache,
            images,
        })
    }
}
