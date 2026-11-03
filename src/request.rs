// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! CLI inputs without execution. Plan and lock locations have no defaults.
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
    /// Build-port appliance entry (request/response CBOR); no flags.
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
    /// Git repository containing the committed build inputs.
    #[arg(default_value = ".")]
    repository: PathBuf,
    #[arg(long)]
    commit: String,
    #[arg(long, default_value = ".")]
    context: PathBuf,
    #[arg(long)]
    plan: PathBuf,
    #[arg(long)]
    lock: PathBuf,
    #[arg(long)]
    cache: PathBuf,
    #[arg(long)]
    output: PathBuf,
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
    plan: ContextPath,
    lock: ContextPath,
    cache: PathBuf,
    output: PathBuf,
    /// Optional provenance locator for build annotations (e.g. appliance CBOR `location`).
    /// Not used as Git identity; mount/path opens still use `repository`.
    location: Option<String>,
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
        plan: ContextPath,
        lock: ContextPath,
        cache: PathBuf,
        output: PathBuf,
    ) -> Self {
        Self {
            id_mappings,
            repository,
            commit,
            context,
            plan,
            lock,
            cache,
            output,
            location: None,
        }
    }

    /// Record an optional build provenance locator without changing Git opens.
    pub(crate) fn with_location(mut self, location: Option<String>) -> Self {
        self.location = location;
        self
    }
    fn from_build_args(raw: BuildArguments) -> Result<Self, RequestError> {
        Ok(Self::from_parts(
            raw.uid_map
                .zip(raw.gid_map)
                .map(|(uid, gid)| IdMappings::new(uid, gid)),
            raw.repository,
            CommitIdentity::try_from(raw.commit.as_str())?,
            ContextDirectory::try_from(raw.context.as_path())?,
            ContextPath::try_from(raw.plan.as_path())?,
            ContextPath::try_from(raw.lock.as_path())?,
            raw.cache,
            raw.output,
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
                    "use the selected entrypoint via main",
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
    pub(crate) fn plan(&self) -> &ContextPath {
        &self.plan
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
    /// Build provenance locator when supplied (appliance CBOR `location`).
    pub(crate) fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use crate::request::BuildRequest;
    use std::path::Path;
    #[derive(Debug)]
    struct Fixture;
    impl Fixture {
        fn args() -> Vec<String> {
            [
                "conglobate",
                "build",
                "repo",
                "--commit",
                "0123456789012345678901234567890123456789",
                "--cache",
                "cache",
                "--output",
                "out",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        }
    }
    #[test]
    fn repository_is_positional_and_defaults_to_current_directory() {
        use super::{Action, Cli};
        use clap::Parser;
        let mut args = Fixture::args();
        args.extend(["--plan", "plan", "--lock", "lock"].map(str::to_owned));
        let Action::Build(explicit) = Cli::try_parse_from(&args).unwrap().action else {
            panic!("expected build");
        };
        assert_eq!(explicit.repository, Path::new("repo"));
        args.remove(2);
        let Action::Build(default) = Cli::try_parse_from(args).unwrap().action else {
            panic!("expected build");
        };
        assert_eq!(default.repository, Path::new("."));
    }
    #[test]
    fn both_selected_configuration_paths_are_required() {
        assert!(BuildRequest::parse_from(Fixture::args()).is_err());
        let mut args = Fixture::args();
        args.extend(["--plan".into(), "build/config.yml".into()]);
        assert!(BuildRequest::parse_from(args.clone()).is_err());
        args.extend(["--lock".into(), "pins/lock.yml".into()]);
        let request = BuildRequest::parse_from(args).unwrap();
        assert_eq!(request.plan().as_ref(), Path::new("build/config.yml"));
        assert_eq!(request.lock().as_ref(), Path::new("pins/lock.yml"));
        assert_eq!(request.context().as_ref(), Path::new("."));
    }
    #[test]
    fn context_selection_does_not_rebase_configuration_against_plan_directory() {
        let mut args = Fixture::args();
        args.extend(
            [
                "--context",
                "project",
                "--plan",
                "plans/custom",
                "--lock",
                "versions",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        let request = BuildRequest::parse_from(args).unwrap();
        assert_eq!(request.context().as_ref(), Path::new("project"));
        assert_eq!(request.lock().as_ref(), Path::new("versions"));
    }
    #[test]
    fn absolute_and_escaping_plan_paths_fail() {
        for path in ["/plan", "../plan", "."] {
            let mut args = Fixture::args();
            args.extend(
                ["--plan", path, "--lock", "lock"]
                    .into_iter()
                    .map(str::to_owned),
            );
            assert!(BuildRequest::parse_from(args).is_err(), "{path}");
        }
    }

    #[test]
    fn environment_id_maps_are_paired_and_validated() {
        let mut args = Fixture::args();
        args.extend(["--plan", "plan", "--lock", "lock"].map(str::to_owned));
        assert!(
            BuildRequest::parse_from(&args)
                .unwrap()
                .id_mappings()
                .is_none()
        );
        for flag in ["--uid-map", "--gid-map"] {
            let mut unpaired = args.clone();
            unpaired.extend([flag, "200000:131072"].map(str::to_owned));
            assert!(BuildRequest::parse_from(unpaired).is_err());
        }
        for invalid in [
            "0:1",
            "1:0",
            "4294967295:1",
            "1:4294967295",
            "200000",
            "1:2:3",
            "-1:2",
            "+1:2",
            " 1:2",
            "1:",
            ":2",
        ] {
            let mut invalid_args = args.clone();
            invalid_args
                .extend(["--uid-map", invalid, "--gid-map", "400000:131072"].map(str::to_owned));
            assert!(BuildRequest::parse_from(invalid_args).is_err(), "{invalid}");
        }
        args.extend(["--uid-map", "200000:131072", "--gid-map", "400000:65536"].map(str::to_owned));
        let mappings = BuildRequest::parse_from(args)
            .unwrap()
            .id_mappings()
            .unwrap();
        assert_eq!(mappings.uid().parent_start(), 200000);
        assert_eq!(mappings.uid().count(), 131072);
        assert_eq!(mappings.gid().parent_start(), 400000);
        assert_eq!(mappings.gid().count(), 65536);
    }

    #[test]
    fn formatter_runtime_flag_is_unknown() {
        let mut args = Fixture::args();
        args.extend(
            [
                "--plan",
                "plan",
                "--lock",
                "lock",
                "--formatter-runtime",
                "/formatter.json",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        let err = BuildRequest::parse_from(args).unwrap_err();
        match err {
            crate::request::RequestError::Cli(error) => {
                let rendered = error.to_string();
                assert!(
                    rendered.contains("unexpected argument")
                        || rendered.contains("formatter-runtime"),
                    "{rendered}"
                );
            }
            other => panic!("expected clap unknown-arg error, got {other:?}"),
        }
    }

    #[test]
    fn appliance_subcommand_is_recognized() {
        use crate::request::{CliCommand, parse_cli};
        match parse_cli(["conglobate", "appliance"]).unwrap() {
            CliCommand::Appliance => {}
            other => panic!("expected Appliance, got {other:?}"),
        }
    }
}
