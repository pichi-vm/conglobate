// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! CLI inputs without execution. Recipe and lock locations have no defaults.
use crate::path::{ContextDirectory, ContextPath, PathError};
use clap::{Args, Parser, Subcommand};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error(transparent)]
    Cli(#[from] clap::Error),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error("commit must be a full 40- or 64-digit hexadecimal object identity")]
    Commit,
}
#[derive(Debug, Parser)]
#[command(name = "conglobate", version)]
struct Cli {
    #[command(subcommand)]
    action: Action,
}
#[derive(Debug, Subcommand)]
enum Action {
    Build(BuildArguments),
}
#[derive(Debug, Args)]
struct BuildArguments {
    #[arg(long)]
    repository: PathBuf,
    #[arg(long)]
    commit: String,
    #[arg(long, default_value = ".")]
    context: PathBuf,
    #[arg(long)]
    recipe: PathBuf,
    #[arg(long)]
    lock: PathBuf,
    #[arg(long)]
    cache: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long)]
    resolv_conf: Option<PathBuf>,
    #[arg(long)]
    formatter_runtime: Option<PathBuf>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitIdentity(Vec<u8>);
impl TryFrom<&str> for CommitIdentity {
    type Error = RequestError;
    fn try_from(text: &str) -> Result<Self, Self::Error> {
        if !matches!(text.len(), 40 | 64) {
            return Err(RequestError::Commit);
        }
        Ok(Self(hex::decode(text).map_err(|_| RequestError::Commit)?))
    }
}
impl CommitIdentity {
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}
#[derive(Debug)]
pub struct BuildRequest {
    repository: PathBuf,
    commit: CommitIdentity,
    context: ContextDirectory,
    recipe: ContextPath,
    lock: ContextPath,
    cache: PathBuf,
    output: PathBuf,
    resolv_conf: Option<PathBuf>,
    formatter_runtime: Option<PathBuf>,
}
impl BuildRequest {
    pub fn parse_from<I, T>(arguments: I) -> Result<Self, RequestError>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let Action::Build(raw) = Cli::try_parse_from(arguments)?.action;
        Ok(Self {
            repository: raw.repository,
            commit: CommitIdentity::try_from(raw.commit.as_str())?,
            context: ContextDirectory::try_from(raw.context.as_path())?,
            recipe: ContextPath::try_from(raw.recipe.as_path())?,
            lock: ContextPath::try_from(raw.lock.as_path())?,
            cache: raw.cache,
            output: raw.output,
            resolv_conf: raw.resolv_conf,
            formatter_runtime: raw.formatter_runtime,
        })
    }
    pub fn repository(&self) -> &Path {
        &self.repository
    }
    pub fn commit(&self) -> &CommitIdentity {
        &self.commit
    }
    pub fn context(&self) -> &ContextDirectory {
        &self.context
    }
    pub fn recipe(&self) -> &ContextPath {
        &self.recipe
    }
    pub fn lock(&self) -> &ContextPath {
        &self.lock
    }
    pub fn cache(&self) -> &Path {
        &self.cache
    }
    pub fn output(&self) -> &Path {
        &self.output
    }
    pub fn formatter_runtime(&self) -> Option<&Path> {
        self.formatter_runtime.as_deref()
    }
    pub fn resolv_conf(&self) -> Option<&Path> {
        self.resolv_conf.as_deref()
    }
}
