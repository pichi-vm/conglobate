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
    #[error("Git ref resolution: {0}")]
    Ref(String),
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
    /// Git reference or full commit hash to build.
    #[arg(long = "ref", value_name = "REF")]
    git_ref: String,
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
    pub(crate) fn resolve(repository: &Path, git_ref: &str) -> Result<Self, RequestError> {
        let repo = gix::open::Options::isolated()
            .open(repository)
            .map_err(|error| RequestError::Ref(error.to_string()))?
            .to_thread_local();
        // Full object id may be supplied directly.
        if let Ok(commit) = Self::try_from(git_ref) {
            let id = gix::ObjectId::from_hex(hex::encode(commit.bytes()).as_bytes())
                .map_err(|error| RequestError::Ref(error.to_string()))?;
            if id.kind() != repo.object_hash() {
                return Err(RequestError::Ref(
                    "commit object format differs from repository".into(),
                ));
            }
            let _ = repo
                .find_object(id)
                .map_err(|error| RequestError::Ref(error.to_string()))?
                .try_into_commit()
                .map_err(|error| RequestError::Ref(error.to_string()))?;
            return Ok(commit);
        }
        let mut reference = repo
            .find_reference(git_ref)
            .map_err(|error| RequestError::Ref(error.to_string()))?;
        let commit = reference
            .peel_to_commit()
            .map_err(|error| RequestError::Ref(error.to_string()))?;
        Self::try_from(commit.id().to_string().as_str())
            .map_err(|error| RequestError::Ref(error.to_string()))
    }

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
        let commit = CommitIdentity::resolve(&raw.repository, &raw.git_ref)?;
        Ok(Self::from_parts(
            raw.uid_map
                .zip(raw.gid_map)
                .map(|(uid, gid)| IdMappings::new(uid, gid)),
            raw.repository,
            commit,
            ContextDirectory::try_from(raw.context.as_path())?,
            ContextPath::try_from(raw.plan.as_path())?,
            ContextPath::try_from(raw.lock.as_path())?,
            raw.cache,
            raw.output,
        ))
    }
    /// Parse CLI arguments, resolve the Git ref and validate context-relative paths.
    /// This opens the repository but does not read the build documents or cache.
    ///
    /// # Errors
    /// Returns CLI diagnostics (including help/version requests), an invalid
    /// Git reference, or a path-validation error. Callers should use the CLI
    /// diagnostic's exit code to distinguish help/version from invalid input.
    /// Test helper: parse argv as a build request only.
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
    struct Fixture(tempfile::TempDir);
    impl Fixture {
        fn new(format: &str) -> Self {
            let fixture = Self(tempfile::tempdir().unwrap());
            fixture.git(&["init", "-b", "main", &format!("--object-format={format}")]);
            fixture.git(&["commit", "--allow-empty", "-m", "fixture"]);
            fixture
        }
        fn git(&self, args: &[&str]) -> String {
            let output = std::process::Command::new("git")
                .args([
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(args)
                .current_dir(self.0.path())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().to_owned()
        }
        fn args(&self) -> Vec<String> {
            [
                "conglobate",
                "build",
                self.0.path().to_str().unwrap(),
                "--ref",
                "HEAD",
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
    fn refs_and_full_hashes_resolve_to_one_commit() {
        for format in ["sha1", "sha256"] {
            let fixture = Fixture::new(format);
            fixture.git(&["tag", "-a", "release", "-m", "release"]);
            let head = fixture.git(&["rev-parse", "HEAD"]);
            let mut args = fixture.args();
            args.extend(["--plan", "plan", "--lock", "lock"].map(str::to_owned));
            for reference in ["HEAD", "main", "refs/heads/main", "release", &head] {
                args[4] = reference.to_owned();
                let request = BuildRequest::parse_from(&args).unwrap();
                assert_eq!(hex::encode(request.commit().bytes()), head);
            }
            args[4] = "main".into();
            let request = BuildRequest::parse_from(&args).unwrap();
            fixture.git(&["commit", "--allow-empty", "-m", "next"]);
            assert_eq!(hex::encode(request.commit().bytes()), head);
            fixture.git(&["checkout", "--detach", &head]);
            args[4] = "HEAD".into();
            assert_eq!(
                hex::encode(BuildRequest::parse_from(&args).unwrap().commit().bytes()),
                head
            );
        }
    }

    #[test]
    fn missing_refs_and_non_commit_objects_are_rejected() {
        for format in ["sha1", "sha256"] {
            let fixture = Fixture::new(format);
            let head = fixture.git(&["rev-parse", "HEAD"]);
            let tree = fixture.git(&["rev-parse", "HEAD^{tree}"]);
            fixture.git(&["tag", "tree", &tree]);
            let wrong_format = "0".repeat(if format == "sha1" { 64 } else { 40 });
            let absent = "0".repeat(head.len());
            let mut args = fixture.args();
            args.extend(["--plan", "plan", "--lock", "lock"].map(str::to_owned));
            for reference in [
                "missing",
                "tree",
                &tree,
                &head[..12],
                &wrong_format,
                &absent,
            ] {
                args[4] = reference.to_owned();
                assert!(BuildRequest::parse_from(&args).is_err(), "{reference}");
            }
            args.drain(3..5);
            assert!(BuildRequest::parse_from(args).is_err());
        }
    }

    #[test]
    fn repository_is_positional_and_defaults_to_current_directory() {
        use super::{Action, Cli};
        use clap::Parser;
        let fixture = Fixture::new("sha1");
        let mut args = fixture.args();
        args.extend(["--plan", "plan", "--lock", "lock"].map(str::to_owned));
        let Action::Build(explicit) = Cli::try_parse_from(&args).unwrap().action else {
            panic!("expected build");
        };
        assert_eq!(explicit.repository, fixture.0.path());
        args.remove(2);
        let Action::Build(default) = Cli::try_parse_from(args).unwrap().action else {
            panic!("expected build");
        };
        assert_eq!(default.repository, Path::new("."));
    }
    #[test]
    fn both_selected_configuration_paths_are_required() {
        let fixture = Fixture::new("sha1");
        assert!(BuildRequest::parse_from(fixture.args()).is_err());
        let mut args = fixture.args();
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
        let fixture = Fixture::new("sha1");
        let mut args = fixture.args();
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
        let fixture = Fixture::new("sha1");
        for path in ["/plan", "../plan", "."] {
            let mut args = fixture.args();
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
        let fixture = Fixture::new("sha1");
        let mut args = fixture.args();
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
        let fixture = Fixture::new("sha1");
        let mut args = fixture.args();
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
