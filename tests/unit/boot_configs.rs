// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::BootConfig;
use crate::{boot_config::BootConfig as Native, contract::plan};
use std::collections::BTreeMap;

fn overlay(base: Option<BootConfig>, value: serde_json::Value) -> anyhow::Result<Native> {
    let values: BTreeMap<plan::Key, Option<plan::Value>> = serde_json::from_value(value)?;
    let config = BootConfig::overlay(
        base,
        values.iter().map(|(key, value)| (key, value.as_ref())),
    )?;
    BootConfig::decode(&config.bytes)
}
#[test]
fn values_are_literal_and_do_not_expand_shell_or_build_variables() {
    for text in [
        "$${CARAPACE} ${CARAPACE} $$",
        "${word}:$1:$2:$99:${missing}",
        "$(touch /outside)",
        "a; b",
    ] {
        let config = overlay(None, serde_json::json!({"kernel.argument":text})).unwrap();
        assert_eq!(config.values()["kernel.argument"], [text]);
    }
}
#[test]
fn overlays_distinguish_bare_empty_list_and_exact_deletion() {
    let base = BootConfig::inherited(
        ruzstd::encoding::compress_to_vec(
            b"corium = present; corium { root = old; child = keep; }\nkernel.console = ttyS0\n"
                .as_slice(),
            ruzstd::encoding::CompressionLevel::Fastest,
        ),
        None,
    )
    .unwrap();
    let result = overlay(Some(base), serde_json::json!({"corium":null,"corium.root":"","kernel.console":["ttyS0","hvc0"],"kernel.quiet":[]})).unwrap();
    assert!(!result.values().contains_key("corium"));
    assert_eq!(result.values()["corium.root"], [""]);
    assert_eq!(result.values()["corium.child"], ["keep"]);
    assert_eq!(result.values()["kernel.console"], ["ttyS0", "hvc0"]);
    assert!(result.values()["kernel.quiet"].is_empty());
}
#[test]
fn invalid_keys_and_unrepresentable_values_are_rejected() {
    for (key, value) in [
        ("kernel.bad key", "text"),
        ("kernel.arg", "nul\0"),
        ("kernel.arg", "both '\" quotes; delimiter"),
    ] {
        assert!(overlay(None, serde_json::json!({key:value})).is_err());
    }
}

#[test]
fn compressed_inheritance_preserves_bytes_and_rejects_broken_frames() {
    let text = b"# retain authored spelling\ncorium.root = old\n";
    let bytes = ruzstd::encoding::compress_to_vec(
        text.as_slice(),
        ruzstd::encoding::CompressionLevel::Fastest,
    );
    let mut inherited = BootConfig::inherited(bytes.clone(), None).unwrap();
    assert_eq!(inherited.bytes, bytes);
    inherited.bind([42; 32]).unwrap();
    assert_eq!(
        BootConfig::decode(&inherited.bytes).unwrap().values()["corium.root"],
        [hex::encode([42; 32])]
    );
    assert!(BootConfig::inherited(text.to_vec(), None).is_err());
    assert!(BootConfig::inherited(bytes[..bytes.len() - 1].to_vec(), None).is_err());
    let mut trailing = bytes;
    trailing.push(0);
    assert!(BootConfig::inherited(trailing, None).is_err());
}

#[test]
fn configuration_size_does_not_depend_on_a_pmi_reservation() {
    let text = "x".repeat(64 * 1024);
    let config = overlay(None, serde_json::json!({"kernel.argument": text})).unwrap();
    assert_eq!(config.values()["kernel.argument"], [text]);
}
