// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! CLI inputs without execution. Recipe and lock locations have no defaults.
use crate::{
    isolation::{IdMappings, IdRange},
    path::{ContextDirectory, ContextPath, PathError},
};
use clap::{Args, Parser, Subcommand};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum RequestError {
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
    Build(Box<BuildArguments>),
    /// One-shot CBOR appliance entry; no flags.
    Appliance,
}
#[derive(Debug, Args)]
struct BuildArguments {
    /// Map image UIDs starting at zero to an environment-reserved parent range.
    #[arg(long, requires = "gid_map", value_name = "PARENT_START:COUNT")]
    uid_map: Option<IdRange>,
    /// Map image GIDs starting at zero to an environment-reserved parent range.
    #[arg(long, requires = "uid_map", value_name = "PARENT_START:COUNT")]
    gid_map: Option<IdRange>,
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
    network_namespace: Option<PathBuf>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommitIdentity(Vec<u8>);
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
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }
}
#[derive(Debug)]
pub(crate) struct BuildRequest {
    id_mappings: Option<IdMappings>,
    repository: PathBuf,
    commit: CommitIdentity,
    context: ContextDirectory,
    recipe: ContextPath,
    lock: ContextPath,
    cache: PathBuf,
    output: PathBuf,
    resolv_conf: Option<PathBuf>,
    network_namespace: Option<PathBuf>,
}
#[derive(Debug)]
pub(crate) enum CliCommand {
    Build(Box<BuildRequest>),
    Appliance,
}

/// Parse argv into the build or appliance product command.
pub(crate) fn parse_cli<I, T>(arguments: I) -> Result<CliCommand, RequestError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match Cli::try_parse_from(arguments)?.action {
        Action::Build(raw) => Ok(CliCommand::Build(Box::new(BuildRequest::from_build_args(
            *raw,
        )?))),
        Action::Appliance => Ok(CliCommand::Appliance),
    }
}

impl BuildRequest {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        id_mappings: Option<IdMappings>,
        repository: PathBuf,
        commit: CommitIdentity,
        context: ContextDirectory,
        recipe: ContextPath,
        lock: ContextPath,
        cache: PathBuf,
        output: PathBuf,
        resolv_conf: Option<PathBuf>,
        network_namespace: Option<PathBuf>,
    ) -> Self {
        Self {
            id_mappings,
            repository,
            commit,
            context,
            recipe,
            lock,
            cache,
            output,
            resolv_conf,
            network_namespace,
        }
    }
    fn from_build_args(raw: BuildArguments) -> Result<Self, RequestError> {
        Ok(Self::from_parts(
            raw.uid_map
                .zip(raw.gid_map)
                .map(|(uid, gid)| IdMappings::new(uid, gid)),
            raw.repository,
            CommitIdentity::try_from(raw.commit.as_str())?,
            ContextDirectory::try_from(raw.context.as_path())?,
            ContextPath::try_from(raw.recipe.as_path())?,
            ContextPath::try_from(raw.lock.as_path())?,
            raw.cache,
            raw.output,
            raw.resolv_conf,
            raw.network_namespace,
        ))
    }
    /// Parse CLI arguments and validate commit syntax and context-relative paths.
    /// This does not open the repository/cache or verify their contents.
    ///
    /// # Errors
    /// Returns CLI diagnostics (including help/version requests), an invalid
    /// commit identity, or a path-validation error. Callers should use the CLI
    /// diagnostic's exit code to distinguish help/version from invalid input.
    /// Test and lifecycle helper: parse argv as a build request only.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn parse_from<I, T>(arguments: I) -> Result<Self, RequestError>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        match parse_cli(arguments)? {
            CliCommand::Build(request) => Ok(*request),
            CliCommand::Appliance => {
                Err(RequestError::Cli(clap::Command::new("conglobate").error(
                    clap::error::ErrorKind::InvalidSubcommand,
                    "use the appliance entrypoint via main",
                )))
            }
        }
    }
    pub(crate) fn id_mappings(&self) -> Option<IdMappings> {
        self.id_mappings
    }
    pub(crate) fn repository(&self) -> &Path {
        &self.repository
    }
    pub(crate) fn commit(&self) -> &CommitIdentity {
        &self.commit
    }
    pub(crate) fn context(&self) -> &ContextDirectory {
        &self.context
    }
    pub(crate) fn recipe(&self) -> &ContextPath {
        &self.recipe
    }
    pub(crate) fn lock(&self) -> &ContextPath {
        &self.lock
    }
    pub(crate) fn cache(&self) -> &Path {
        &self.cache
    }
    pub(crate) fn output(&self) -> &Path {
        &self.output
    }
    pub(crate) fn network_namespace(&self) -> Option<&Path> {
        self.network_namespace.as_deref()
    }
    pub(crate) fn resolv_conf(&self) -> Option<&Path> {
        self.resolv_conf.as_deref()
    }
}
