// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ImageRef, Remote};
use serde::{Deserialize, Serialize};

/// A creation stage's tool carapace, optionally resolved without a lock pin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tool {
    #[serde(rename = "bootstrap")]
    Bootstrap(Remote),

    #[serde(untagged)]
    Image(ImageRef),
}

#[cfg(all(test, feature = "yaml"))]
mod tests {
    use super::*;
    use crate::plan::{Plan, Stage};

    #[test]
    fn bootstrap_tools_preserve_tags_aliases_and_external_names() {
        for reference in [
            "localhost/fedora",
            "ghcr.io/pichi-vm/fedora",
            "localhost:5000/fedora:43",
        ] {
            let text = format!(
                "version: 1\nstages:\n  root: {{use: &seed !bootstrap {reference}}}\n  other: {{use: *seed}}\nimage: root\n"
            );
            let plan = Plan::from_yaml(&text).unwrap();
            for stage in plan.stages.values() {
                assert!(matches!(
                    stage,
                    Stage::Create {
                        r#use: Tool::Bootstrap(_),
                        ..
                    }
                ));
            }
        }
        Plan::from_yaml("version: 1\nimage: ghcr.io/pichi-vm/fedora").unwrap();
    }

    #[test]
    fn bootstrap_is_valid_only_as_an_external_creation_tool() {
        for body in [
            "stages: {root: {use: !bootstrap local}}\nimage: root",
            "stages: {root: {extend: !bootstrap localhost/fedora}}\nimage: root",
            "image: !bootstrap localhost/fedora",
            "image: {root: !bootstrap localhost/fedora}",
            "image: {boot: !bootstrap localhost/fedora}",
            "image: {boot: {inherit: !bootstrap localhost/fedora}}",
            "image: {boot: {extract: {from: !bootstrap localhost/fedora, path: /x}}}",
            "stages: {root: {use: ghcr.io/pichi-vm/fedora, steps: [{copy: {from: !bootstrap localhost/fedora, path: /x, into: /x}}]}}\nimage: root",
            "stages: {root: {use: localhost/fedora}}\nimage: root",
            "stages: {root: {extend: localhost:5000/fedora}}\nimage: root",
            "image: localhost/fedora",
            "stages: {root: {use: !bootstrap team/fedora}}\nimage: root",
            "stages: {root: {use: !bootstrap [localhost/fedora]}}\nimage: root",
            "stages: {root: {use: !bootstrap {image: localhost/fedora}}}\nimage: root",
        ] {
            let text = format!("version: 1\n{body}\n");
            assert!(Plan::from_yaml(&text).is_err(), "{text}");
        }
    }

    #[test]
    fn unknown_and_misplaced_tags_cannot_be_silently_discarded() {
        for text in [
            "!bootstrap {version: 1, image: ghcr.io/pichi-vm/fedora}",
            "version: !bootstrap 1\nimage: ghcr.io/pichi-vm/fedora",
            "version: 1\n!bootstrap image: ghcr.io/pichi-vm/fedora",
            "version: 1\nstages: {!bootstrap root: {use: ghcr.io/pichi-vm/fedora}}\nimage: root",
            "version: 1\nstages: {root: {!bootstrap use: ghcr.io/pichi-vm/fedora}}\nimage: root",
            "version: 1\nstages: {root: {use: !unknown ghcr.io/pichi-vm/fedora}}\nimage: root",
            "!<tag:example.com,2026:unknown> {version: 1, image: ghcr.io/pichi-vm/fedora}",
            "version: 1\nstages: {root: {use: !<tag:example.com,2026:unknown> ghcr.io/pichi-vm/fedora}}\nimage: root",
        ] {
            assert!(Plan::from_yaml(text).is_err(), "{text}");
        }
    }
}
