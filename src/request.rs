// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! CLI inputs without execution. Plan and lock locations have no defaults.
pub(crate) use crate::commit::CommitIdentity;
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
    #[error(transparent)]
    Commit(#[from] crate::commit::CommitError),

    #[error("build location must be a credential-free Git locator")]
    Location,
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
    /// Run the build request/result protocol with caller-supplied resources.
    Run(RuntimeArguments),
}
#[derive(Debug, Args)]
struct BuildArguments {
    #[command(flatten)]
    runtime: RuntimeArguments,
    /// Git reference or full commit hash to build.
    #[arg(long = "ref", value_name = "REF")]
    git_ref: String,
    #[arg(long, default_value = ".")]
    context: PathBuf,
    #[arg(long)]
    plan: PathBuf,
    #[arg(long)]
    lock: PathBuf,

    /// Optional credential-free source locator recorded in build annotations.
    #[arg(long)]
    location: Option<String>,
}
#[derive(Debug, Args)]
pub(crate) struct RuntimeArguments {
    /// Map image UIDs starting at zero to an environment-reserved parent range.
    #[arg(long, requires = "gid_map", value_name = "PARENT_START:COUNT")]
    pub(crate) uid_map: Option<IdRange>,
    /// Map image GIDs starting at zero to an environment-reserved parent range.
    #[arg(long, requires = "uid_map", value_name = "PARENT_START:COUNT")]
    pub(crate) gid_map: Option<IdRange>,
    /// Git repository containing the committed build inputs.
    #[arg(default_value = ".")]
    pub(crate) repository: PathBuf,
    #[arg(long)]
    pub(crate) cache: PathBuf,
    #[arg(long)]
    pub(crate) output: PathBuf,
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
    /// Optional provenance locator for build annotations (e.g. terminal CBOR `location`).
    /// Not used as Git identity; mount/path opens still use `repository`.
    location: Option<String>,
}
#[derive(Debug)]
pub(crate) enum CliCommand {
    Build(Box<BuildRequest>),
    Run(RuntimeArguments),
}

/// Parse argv into the selected build entry point.
pub(crate) fn parse_cli<I, T>(arguments: I) -> Result<CliCommand, RequestError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match Cli::try_parse_from(arguments)?.action {
        Action::Build(raw) => Ok(CliCommand::Build(Box::new(BuildRequest::from_build_args(
            *raw,
        )?))),
        Action::Run(args) => Ok(CliCommand::Run(args)),
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
    pub(crate) fn with_location(mut self, location: Option<String>) -> Result<Self, RequestError> {
        if let Some(location) = &location {
            // URL parse errors may include credentials; return a fixed diagnostic.
            let locator =
                gix::url::parse(location.as_bytes().into()).map_err(|_| RequestError::Location)?;
            if locator.password().is_some()
                || (locator.user().is_some() && locator.scheme != gix::url::Scheme::Ssh)
            {
                return Err(RequestError::Location);
            }
        }
        self.location = location;
        Ok(self)
    }
    /// Combine protocol input with caller resources using the CLI validation path.
    pub(crate) fn from_terminal_input(
        input: crate::contract::launch::Input,
        runtime: RuntimeArguments,
    ) -> Result<Self, RequestError> {
        Self::from_build_args(BuildArguments {
            runtime,
            git_ref: input.r#ref,
            context: input.context.into(),
            plan: input.plan.into(),
            lock: input.lock.into(),
            location: input.location,
        })
    }
    fn from_build_args(raw: BuildArguments) -> Result<Self, RequestError> {
        let commit = CommitIdentity::resolve(&raw.runtime.repository, &raw.git_ref)?;
        Self::from_parts(
            raw.runtime
                .uid_map
                .zip(raw.runtime.gid_map)
                .map(|(uid, gid)| IdMappings::new(uid, gid)),
            raw.runtime.repository,
            commit,
            ContextDirectory::try_from(raw.context.as_path())?,
            ContextPath::try_from(raw.plan.as_path())?,
            ContextPath::try_from(raw.lock.as_path())?,
            raw.runtime.cache,
            raw.runtime.output,
        )
        .with_location(raw.location)
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
            CliCommand::Run(_) => Err(RequestError::Cli(clap::Command::new("conglobate").error(
                clap::error::ErrorKind::InvalidSubcommand,
                "use the selected entrypoint via main",
            ))),
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
    /// Build provenance locator when supplied (terminal CBOR `location`).
    pub(crate) fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use crate::request::BuildRequest;
    use clap::Parser;
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
    fn terminal_and_cli_share_launch_argument_validation() {
        use crate::contract::launch::Input;
        let fixture = Fixture::new("sha1");
        let input = Input {
            r#ref: "refs/heads/main".into(),
            plan: "plan.yml".into(),
            lock: "lock.yml".into(),
            context: "project".into(),
            location: Some("https://example.invalid/repo".into()),
        };
        let compare = |input: Input| {
            let mut args = vec![
                "conglobate",
                "build",
                fixture.0.path().to_str().unwrap(),
                "--ref",
                &input.r#ref,
                "--plan",
                &input.plan,
                "--lock",
                &input.lock,
                "--context",
                &input.context,
                "--cache",
                "cache",
                "--output",
                "out",
                "--uid-map",
                "120000:32000",
                "--gid-map",
                "240000:16000",
            ];
            if let Some(location) = input.location.as_deref() {
                args.extend(["--location", location]);
            }
            let cli = BuildRequest::parse_from(args);
            let terminal = BuildRequest::from_terminal_input(
                input,
                super::RuntimeArguments {
                    repository: fixture.0.path().to_path_buf(),
                    cache: "cache".into(),
                    output: "out".into(),
                    uid_map: Some("120000:32000".parse().unwrap()),
                    gid_map: Some("240000:16000".parse().unwrap()),
                },
            );
            match (cli, terminal) {
                (Ok(cli), Ok(terminal)) => {
                    assert_eq!(cli.commit(), terminal.commit());
                    assert_eq!(cli.repository(), terminal.repository());
                    assert_eq!(cli.context(), terminal.context());
                    assert_eq!(cli.plan(), terminal.plan());
                    assert_eq!(cli.lock(), terminal.lock());
                    assert_eq!(cli.location(), terminal.location());
                    assert_eq!(cli.cache(), terminal.cache());
                    assert_eq!(cli.output(), terminal.output());
                    for (a, b) in [
                        (
                            cli.id_mappings().unwrap().uid(),
                            terminal.id_mappings().unwrap().uid(),
                        ),
                        (
                            cli.id_mappings().unwrap().gid(),
                            terminal.id_mappings().unwrap().gid(),
                        ),
                    ] {
                        assert_eq!(a.first(), b.first());
                        assert_eq!(a.count(), b.count());
                    }
                    true
                }
                (Err(cli), Err(terminal)) => {
                    assert_eq!(cli.to_string(), terminal.to_string());
                    false
                }
                other => panic!("entry points disagree: {other:?}"),
            }
        };
        assert!(compare(input.clone()));
        let mut without_location = input.clone();
        without_location.location = None;
        assert!(compare(without_location));
        for path in ["../escape", "/etc/passwd"] {
            let mut invalid = input.clone();
            invalid.plan = path.into();
            assert!(!compare(invalid));
            let mut invalid = input.clone();
            invalid.lock = path.into();
            assert!(!compare(invalid));
            let mut invalid = input.clone();
            invalid.context = path.into();
            assert!(!compare(invalid));
        }
        for reference in [
            "missing".to_owned(),
            fixture.git(&["rev-parse", "HEAD^{tree}"]),
        ] {
            let mut invalid = input.clone();
            invalid.r#ref = reference;
            assert!(!compare(invalid));
        }
        let mut unmapped_args = fixture.args();
        unmapped_args.extend(["--plan", "plan.yml", "--lock", "lock.yml"].map(str::to_owned));
        let cli = BuildRequest::parse_from(unmapped_args).unwrap();
        let unmapped = BuildRequest::from_terminal_input(
            input.clone(),
            super::RuntimeArguments {
                repository: fixture.0.path().to_path_buf(),
                cache: "cache".into(),
                output: "out".into(),
                uid_map: None,
                gid_map: None,
            },
        )
        .unwrap();
        assert!(cli.id_mappings().is_none() && unmapped.id_mappings().is_none());
        assert_eq!(cli.commit(), unmapped.commit());
        let mut invalid = input;
        invalid.location = Some("https://user:secret@example.invalid/repo".into());
        assert!(!compare(invalid));
    }

    #[test]
    fn location_is_optional_and_rejects_credentials_without_echoing_them() {
        let fixture = Fixture::new("sha1");
        let mut args = fixture.args();
        args.extend(["--plan", "plan", "--lock", "lock"].map(str::to_owned));
        assert!(
            BuildRequest::parse_from(&args)
                .unwrap()
                .location()
                .is_none()
        );
        for location in [
            "https://example.invalid/repo",
            "ssh://git@example.invalid/repo",
        ] {
            let mut located = args.clone();
            located.extend(["--location", location].map(str::to_owned));
            assert_eq!(
                BuildRequest::parse_from(located).unwrap().location(),
                Some(location)
            );
        }
        for location in [
            "https://secret@example.invalid/repo",
            "https://user:secret@example.invalid/repo",
            "ssh://user:secret@example.invalid/repo",
        ] {
            let mut located = args.clone();
            located.extend(["--location", location].map(str::to_owned));
            let error = BuildRequest::parse_from(located).unwrap_err().to_string();
            assert!(error.contains("credential-free"));
            assert!(!error.contains("secret"));
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
        assert_eq!(explicit.runtime.repository, fixture.0.path());
        args.remove(2);
        let Action::Build(default) = Cli::try_parse_from(args).unwrap().action else {
            panic!("expected build");
        };
        assert_eq!(default.runtime.repository, Path::new("."));
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
    fn run_runtime_arguments_match_build_defaults_and_mapping_rules() {
        let fixture = Fixture::new("sha1");
        for command in ["build", "run"] {
            let mut args = vec!["conglobate", command, "--cache=cache", "--output=out"];
            if command == "build" {
                args.extend(["--ref=HEAD", "--plan=plan", "--lock=lock"]);
            }
            let runtime = |args: Vec<&str>| {
                let cli = super::Cli::try_parse_from(args).unwrap();
                match cli.action {
                    super::Action::Build(args) => args.runtime,
                    super::Action::Run(args) => args,
                }
            };
            let defaults = runtime(args.clone());
            assert_eq!(defaults.repository, Path::new("."));
            assert!(defaults.uid_map.is_none() && defaults.gid_map.is_none());
            let mut mapped = args.clone();
            mapped.extend(["--uid-map=120000:32000", "--gid-map=240000:16000"]);
            let mapped = runtime(mapped);
            assert_eq!(mapped.uid_map.unwrap().count(), 32000);
            assert_eq!(mapped.gid_map.unwrap().first(), 240000);
            for maps in [
                vec!["--uid-map=120000:32000"],
                vec!["--gid-map=240000:16000"],
                vec!["--uid-map=0:65536", "--gid-map=240000:16000"],
            ] {
                let mut invalid = args.clone();
                invalid.extend(maps);
                assert!(super::Cli::try_parse_from(invalid).is_err());
            }
            for absent in ["--cache=cache", "--output=out"] {
                assert!(
                    super::Cli::try_parse_from(args.iter().filter(|arg| **arg != absent)).is_err()
                );
            }
        }
        let run = [
            "conglobate",
            "run",
            fixture.0.path().to_str().unwrap(),
            "--cache=cache",
            "--output=out",
        ];
        assert!(matches!(
            super::parse_cli(run).unwrap(),
            super::CliCommand::Run(_)
        ));
        for flag in [
            "--ref=HEAD",
            "--context=.",
            "--plan=plan",
            "--lock=lock",
            "--location=https://example.invalid/repo",
            "--port-name=caller.build",
        ] {
            let mut invalid = run.to_vec();
            invalid.push(flag);
            assert!(super::parse_cli(invalid).is_err());
        }
        let mut old_name = run;
        old_name[1] = "terminal";
        assert!(super::parse_cli(old_name).is_err());
    }

    #[test]
    fn appliance_subcommand_is_rejected() {
        assert!(super::parse_cli(["conglobate", "appliance"]).is_err());
    }
}
