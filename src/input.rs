// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Build planning from one immutable committed context. Image verification follows.
use crate::{
    context::{CapturedContext, ContextError, ContextFile},
    document::YamlDocument,
    graph::BuildGraph,
    lock::{ImageLock, LockError},
    path::ContextPath,
    plan::{Plan, PlanError},
    request::BuildRequest,
};
#[cfg(test)]
use sha2::{Digest, Sha256};

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
    #[cfg(test)]
    sha256: [u8; 32],
}
impl SelectedDocument {
    fn new(path: ContextPath, _file: &ContextFile) -> Self {
        Self {
            path,
            #[cfg(test)]
            sha256: Sha256::digest(_file.bytes()).into(),
        }
    }
    pub(crate) fn path(&self) -> &ContextPath {
        &self.path
    }
    #[cfg(test)]
    pub(crate) fn sha256(&self) -> &[u8; 32] {
        &self.sha256
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
    networking: Option<crate::network::NetworkInput>,
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
                if let crate::plan::Directive::Copy(copy) = directive
                    && !artifacts.contains(copy.artifact())
                {
                    crate::copy::CopySources::prepare(&mut context, copy.sources())?;
                }
            }
        }

        let networking = if graph.network_steps().is_empty() {
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
        let id_mappings = if graph.execution_required() {
            Some(
                request
                    .id_mappings()
                    .ok_or(InputError::IdMappingsRequired)?,
            )
        } else {
            None
        };
        let plan = SelectedDocument::new(request.plan().clone(), &plan_file);
        let lock = SelectedDocument::new(request.lock().clone(), &lock_file);
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
    #[cfg(test)]
    pub(crate) fn id_mappings(&self) -> Option<crate::isolation::IdMappings> {
        self.id_mappings
    }
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
    pub(crate) fn networking(&self) -> Option<&crate::network::NetworkInput> {
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
                crate::plan::Reference::Local(local) => *exports
                    .get(local)
                    .ok_or_else(|| InputError::Exports("unresolved local input".into()))?,
                crate::plan::Reference::Remote(remote) => {
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
                crate::plan::TaskKind::Bootstrap(_) => (true, false),
                crate::plan::TaskKind::Extend => input,
                crate::plan::TaskKind::Build => (false, task.firmware().is_some()),
            };
            exports.insert(name.clone(), output);
        }
        match self.graph.plan().image() {
            crate::plan::Image::Short(name) => {
                let (root, boot) = exports[name];
                if !root && !boot {
                    return Err(InputError::Exports("image has no component exports".into()));
                }
            }
            crate::plan::Image::Long { root, boot } if !exports[root].0 || !exports[boot].1 => {
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
