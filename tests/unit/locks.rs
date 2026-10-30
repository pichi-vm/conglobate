// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{document::YamlDocument, lock::ImageLock};

#[test]
fn pins_are_exact_top_level_identities() {
    let text = format!(
        "version: 1\nimages:\n  example.test/base: {{index: sha256:{}}}\n",
        "ab".repeat(32)
    );
    let lock = ImageLock::try_from(YamlDocument::from(text.as_str())).unwrap();
    assert_eq!(
        lock.resolve(&"example.test/base".parse().unwrap())
            .unwrap()
            .bytes(),
        &[0xab; 32]
    );
    assert!(
        lock.resolve(&"example.test/missing".parse().unwrap())
            .is_err()
    );
}

#[test]
fn metadata_lock_pins_the_documented_fixture_index() {
    use sha2::{Digest as _, Sha256};
    let text = include_str!("fixtures/metadata-lock.yaml");
    let bytes = text
        .lines()
        .find_map(|line| line.strip_prefix("# index-bytes: "))
        .expect("documented fixture index");
    let mut hash = Sha256::new();
    hash.update(bytes.as_bytes());
    let lock = ImageLock::try_from(YamlDocument::from(text)).unwrap();
    let pin = lock
        .resolve(&"qualification.local/metadata:1".parse().unwrap())
        .unwrap();
    assert_eq!(pin.bytes().as_slice(), hash.finalize().as_slice());
}

#[test]
fn duplicate_references_and_unknown_fields_are_rejected() {
    let pin = format!("{{index: sha256:{}}}", "00".repeat(32));
    for text in [
        format!("version: 1\nimages:\n  base: {pin}\n  base: {pin}"),
        format!("version: 1\nversion: 1\nimages: {{base: {pin}}}"),
        "version: 1\nimages: {}\nextra: ignored".into(),
        "version: 1\nimages: null".into(),
        format!("version: 1\nimages: {{base: {pin}}}"),
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
