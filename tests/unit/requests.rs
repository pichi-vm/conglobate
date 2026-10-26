// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::request::BuildRequest;
use std::path::Path;
#[derive(Debug)]
struct Fixture;
impl Fixture {
    fn args() -> Vec<String> {
        [
            "conglobate",
            "build",
            "--repository",
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
fn both_selected_configuration_paths_are_required() {
    assert!(BuildRequest::parse_from(Fixture::args()).is_err());
    let mut args = Fixture::args();
    args.extend(["--recipe".into(), "build/config.yml".into()]);
    assert!(BuildRequest::parse_from(args.clone()).is_err());
    args.extend(["--lock".into(), "pins/lock.yml".into()]);
    let request = BuildRequest::parse_from(args).unwrap();
    assert_eq!(request.recipe().as_ref(), Path::new("build/config.yml"));
    assert_eq!(request.lock().as_ref(), Path::new("pins/lock.yml"));
    assert_eq!(request.context().as_ref(), Path::new(""));
}
#[test]
fn context_selection_does_not_rebase_configuration_against_recipe_directory() {
    let mut args = Fixture::args();
    args.extend(
        [
            "--context",
            "project",
            "--recipe",
            "recipes/custom",
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
fn absolute_and_escaping_recipe_paths_fail() {
    for path in ["/recipe", "../recipe", "."] {
        let mut args = Fixture::args();
        args.extend(
            ["--recipe", path, "--lock", "lock"]
                .into_iter()
                .map(str::to_owned),
        );
        assert!(BuildRequest::parse_from(args).is_err(), "{path}");
    }
}

#[test]
fn environment_id_maps_are_paired_and_validated() {
    let mut args = Fixture::args();
    args.extend(["--recipe", "recipe", "--lock", "lock"].map(str::to_owned));
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
            "--recipe",
            "recipe",
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
        conglobate::request::RequestError::Cli(error) => {
            let rendered = error.to_string();
            assert!(
                rendered.contains("unexpected argument") || rendered.contains("formatter-runtime"),
                "{rendered}"
            );
        }
        other => panic!("expected clap unknown-arg error, got {other:?}"),
    }
}
