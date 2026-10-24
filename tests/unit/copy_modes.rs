// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::copy::CopyMode;

#[test]
fn recipes_validate_modes_and_multiple_source_destinations() {
    use conglobate::{document::YamlDocument, recipe::Recipe};
    for mode in ["0644", "u=rw,g=u,o=r", "a+X", "u+x-w", "a+t", "a=", "u=go"] {
        let yaml = format!(
            "carapace: {{from: base, derive: [{{copy: {{from: file, into: /file, mode: '{mode}'}}}}]}}"
        );
        assert!(
            Recipe::try_from(YamlDocument::from(yaml.as_str())).is_ok(),
            "{mode}"
        );
    }
    for copy in [
        "from: [a, b], into: /directory",
        "from: a, into: /a, mode: '888'",
    ] {
        let yaml = format!("carapace: {{from: base, derive: [{{copy: {{{copy}}}}}]}}");
        assert!(
            Recipe::try_from(YamlDocument::from(yaml.as_str())).is_err(),
            "{copy}"
        );
    }
    let yaml = "carapace: {from: base, derive: [{copy: {from: [a, b], into: /directory/}}]}";
    assert!(Recipe::try_from(YamlDocument::from(yaml)).is_ok());
}
#[test]
fn invalid_modes_fail_before_filesystem_execution() {
    for text in [
        "", "888", "10000", "+755", "-755", "u", "a+z", "a+r,", "u+ r", "0o755",
    ] {
        assert!(text.parse::<CopyMode>().is_err(), "{text}");
    }
}
