// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{copy::SourcePattern, document::YamlDocument, recipe::Recipe};
use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

#[test]
fn linux_filepath_patterns_cover_unicode_classes_and_literal_braces() {
    for (pattern, name, expected) in [
        ("?", "é", true),
        ("??", "é", false),
        ("[à-ê]", "é", true),
        ("[!a]", "!", true),
        ("[!a]", "b", false),
        ("[^a]", "b", true),
        ("*{a,b}", "xa", false),
        ("*{a,b}", "x{a,b}", true),
        ("arr[[]0].txt", "arr[0].txt", true),
        (r"a\*", "a*", true),
        ("[z-a]", "z", false),
        ("**", ".hidden", true),
        ("**", "dir/file", false),
        ("dir/?", "dir/é", true),
        ("dir/*", "dir/a/b", false),
        ("/dir/./file/", "dir/file", true),
        (".", "", true),
    ] {
        assert_eq!(
            pattern
                .parse::<SourcePattern>()
                .unwrap()
                .matches(OsStr::new(name)),
            expected,
            "{pattern} {name}"
        );
    }
    assert!(
        "?".parse::<SourcePattern>()
            .unwrap()
            .matches(OsStr::from_bytes(b"\xff"))
    );
    assert!(
        "??".parse::<SourcePattern>()
            .unwrap()
            .matches(OsStr::from_bytes(b"\xff\xfe"))
    );
}

#[test]
fn malformed_patterns_and_parent_traversal_fail_during_recipe_parsing() {
    for source in ["[", "[]", "[a-]", "\\", "[^]", "../file", "dir/../file", ""] {
        let yaml = format!(
            "carapace: {{from: base, derive: [{{copy: {{from: '{source}', into: /file}}}}]}}"
        );
        assert!(
            Recipe::try_from(YamlDocument::from(yaml.as_str())).is_err(),
            "{source:?}"
        );
    }
}
