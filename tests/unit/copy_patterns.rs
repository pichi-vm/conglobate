// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{copy::SourcePattern, document::YamlDocument, plan::Plan};
use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

struct SmallGlob;
impl SmallGlob {
    // Independent recursive definition, used only on inputs of at most four
    // tokens/runes. Production uses a forward state vector instead.
    fn matches(pattern: &[u8], name: &[u8]) -> bool {
        match pattern.split_first() {
            None => name.is_empty(),
            Some((b'*', rest)) => (0..=name.len()).any(|n| Self::matches(rest, &name[n..])),
            Some((token, rest)) => name.split_first().is_some_and(|(first, tail)| {
                (*token == b'?' || token == first) && Self::matches(rest, tail)
            }),
        }
    }

    fn words(alphabet: &[u8], length: usize) -> Vec<Vec<u8>> {
        let mut words = vec![Vec::new()];
        for _ in 0..length {
            words = words
                .into_iter()
                .flat_map(|prefix| {
                    alphabet.iter().map(move |byte| {
                        let mut word = prefix.clone();
                        word.push(*byte);
                        word
                    })
                })
                .collect();
        }
        words
    }
}

#[test]
fn short_globs_agree_with_independent_rune_sequence_definition() {
    let names: Vec<_> = (0..=4).flat_map(|n| SmallGlob::words(b"abXY", n)).collect();
    for length in 1..=4 {
        for pattern in SmallGlob::words(b"ab*?", length) {
            let text = std::str::from_utf8(&pattern).unwrap();
            let compiled = text.parse::<SourcePattern>().unwrap();
            for name in &names {
                // X is one Unicode rune; Y is one invalid UTF-8 byte. Neither
                // can match literal a/b, but each must consume exactly one ?.
                let bytes: Vec<u8> = name
                    .iter()
                    .flat_map(|byte| match byte {
                        b'X' => "é".as_bytes().to_vec(),
                        b'Y' => vec![0xff],
                        byte => vec![*byte],
                    })
                    .collect();
                assert_eq!(
                    compiled.matches(OsStr::from_bytes(&bytes)),
                    SmallGlob::matches(&pattern, name),
                    "pattern {text:?}, name {bytes:?}"
                );
            }
        }
    }
}

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
fn malformed_patterns_and_parent_traversal_fail_during_plan_parsing() {
    for source in ["[", "[]", "[a-]", "\\", "[^]", "../file", "dir/../file", ""] {
        let yaml = format!(
            "image: root\ntasks: {{root: {{kind: extend, from: registry.test/base:1, exec: [{{copy: {{from: '{source}', into: /file}}}}]}}}}"
        );
        assert!(
            Plan::try_from(YamlDocument::from(yaml.as_str())).is_err(),
            "{source:?}"
        );
    }
}
