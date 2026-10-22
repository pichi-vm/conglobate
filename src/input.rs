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
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error(transparent)]
    Recipe(#[from] RecipeError),
    #[error(transparent)]
    Lock(#[from] LockError),
    #[error("network-enabled commands require --resolv-conf")]
    ResolverRequired,
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
        if !plan.network_steps().is_empty() && request.resolv_conf().is_none() {
            return Err(InputError::ResolverRequired);
        }
        let recipe = SelectedDocument::new(request.recipe().clone(), &recipe_file);
        let lock = SelectedDocument::new(request.lock().clone(), &lock_file);
        Ok(Self {
            context,
            recipe,
            lock,
            plan,
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
    pub fn plan(&self) -> &BuildPlan {
        &self.plan
    }
}
