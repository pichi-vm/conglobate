// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Build planning from one immutable committed context. Image verification follows.
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
    #[error("executing stages require --uid-map and --gid-map")]
    IdMappingsRequired,
    #[error(transparent)]
    Cache(#[from] crate::cache::CacheError),
    #[error("{0}")]
    Exports(String),
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
        for task in graph.plan().tasks().values() {
            for name in task.outputs().keys() {
                if context
                    .entries()
                    .contains_key(std::path::Path::new(name.as_ref()))
                {
                    return Err(InputError::Exports(format!(
                        "artifact {name} conflicts with committed context"
                    )));
                }
            }
        }

        let artifacts: std::collections::BTreeSet<_> = graph
            .plan()
            .tasks()
            .values()
            .flat_map(|task| task.outputs().keys().map(|name| name.as_ref()))
            .collect();
        for name in graph.plan().order() {
            for directive in graph.plan().tasks()[name].directives() {
                if let crate::planner::Directive::Copy(copy) = directive
                    && !artifacts.contains(copy.artifact())
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
        let id_mappings = if graph.execution_required() {
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
            cache.verify_metadata(&image)?;
            if input.execution_required() && image.root().is_none() {
                return Err(InputError::MissingExecutionCarapace(
                    input.reference().as_ref().to_owned(),
                ));
            }
            images.insert(input.reference().clone(), CheckedImage { image });
        }
        // Verify export availability before allocating any privileged execution
        // resources. A missing optional boot is fine except when image requests it.
        let mut exports = std::collections::BTreeMap::new();
        for name in self.graph.plan().order() {
            let task = &self.graph.plan().tasks()[name];
            let input = match task.from() {
                crate::planner::Reference::Local(local) => *exports
                    .get(local)
                    .ok_or_else(|| InputError::Exports("unresolved local input".into()))?,
                crate::planner::Reference::Remote(remote) => {
                    let image = &images[&remote.as_ref().parse().map_err(PlanError::from)?];
                    (
                        image.root().is_some(),
                        image
                            .image()
                            .manifest()
                            .layers
                            .iter()
                            .any(|l| matches!(l, crate::artifact::Layer::Pmi(_))),
                    )
                }
            };
            if !input.0 {
                return Err(InputError::Exports(format!(
                    "task {name} requires a carapace input"
                )));
            }
            let output = match task.kind() {
                crate::planner::TaskKind::Bootstrap(_) => (true, false),
                crate::planner::TaskKind::Extend => input,
                crate::planner::TaskKind::Build => (false, task.firmware().is_some()),
            };
            exports.insert(name.clone(), output);
        }
        match self.graph.plan().image() {
            crate::planner::Image::Short(name) => {
                let (root, boot) = exports[name];
                if !root && !boot {
                    return Err(InputError::Exports("image has no component exports".into()));
                }
            }
            crate::planner::Image::Long { root, boot } if !exports[root].0 || !exports[boot].1 => {
                return Err(InputError::Exports(
                    "requested image root or boot export unavailable".into(),
                ));
            }
            _ => (),
        }
        Ok(ResolvedBuild {
            captured: self,
            cache,
            images,
        })
    }
}
