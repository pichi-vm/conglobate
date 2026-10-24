// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{document::YamlDocument, lock::ImageLock};

#[test]
fn pins_are_exact_top_level_identities() {
    let text = format!(
        "version: 1\nimages:\n  example/base: {{index: sha256:{}}}\n",
        "ab".repeat(32)
    );
    let lock = ImageLock::try_from(YamlDocument::from(text.as_str())).unwrap();
    assert_eq!(
        lock.resolve(&"example/base".parse().unwrap())
            .unwrap()
            .bytes(),
        &[0xab; 32]
    );
    assert!(lock.resolve(&"missing".parse().unwrap()).is_err());
}

#[test]
fn duplicate_references_and_unknown_fields_are_rejected() {
    let pin = format!("{{index: sha256:{}}}", "00".repeat(32));
    for text in [
        format!("version: 1\nimages:\n  base: {pin}\n  base: {pin}"),
        format!("version: 1\nversion: 1\nimages: {{base: {pin}}}"),
        "version: 1\nimages: {}\nextra: ignored".into(),
        "version: 2\nimages: {}".into(),
        "version: 1\nimages: {base: {index: latest}}".into(),
        "version: 1\nimages: {base: {index: sha256:00, manifest: other}}".into(),
    ] {
        assert!(
            ImageLock::try_from(YamlDocument::from(text.as_str())).is_err(),
            "{text}"
        );
    }
}
