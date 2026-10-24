// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::template::Template;

#[test]
fn expansion_is_single_pass_and_dollar_escaping_is_consistent() {
    let template = Template::set("$${CARAPACE} ${CARAPACE} $$").unwrap();
    assert_eq!(
        template.expand(Some("$literal"), None).unwrap(),
        "${CARAPACE} $literal $"
    );
    assert!(
        Template::set("${CARAPACE}")
            .unwrap()
            .expand(None, None)
            .is_err()
    );
    assert!(
        Template::set("${UNKNOWN}")
            .unwrap()
            .expand(Some("root"), None)
            .is_err()
    );
}

#[test]
fn captures_use_regex_semantics_and_do_not_become_build_variables() {
    let regex = regex::Regex::new("(?P<word>abc)(def)?").unwrap();
    let captures = regex.captures("abc").unwrap();
    let template = Template::replacement("${word}:$1:$2:$99:${missing}:${CARAPACE}").unwrap();
    assert_eq!(
        template.expand(Some("root"), Some(&captures)).unwrap(),
        "abc:abc::::root"
    );
    // Merely parsing does not evaluate the unavailable root.
    assert!(Template::replacement("${CARAPACE}").is_ok());
}

#[test]
fn invalid_templates_fail_before_execution() {
    for source in ["$", "${}", "${bad-name}", "${CARAPACE", "$!", "nul\0"] {
        assert!(Template::replacement(source).is_err(), "{source:?}");
    }
    for source in ["$1", "${word}"] {
        assert!(Template::set(source).is_err(), "{source}");
    }
}
