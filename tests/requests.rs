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
