// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::BootConfig;
use crate::contract::{artifact::BootConfig as Native, plan};
use std::collections::BTreeMap;

fn overlay(base: Option<BootConfig>, value: serde_json::Value) -> anyhow::Result<Native> {
    let values: BTreeMap<plan::boot_config::Key, Option<plan::boot_config::Value>> =
        serde_json::from_value(value)?;
    let config = BootConfig::overlay(
        base,
        values.iter().map(|(key, value)| (key, value.as_ref())),
    )?;
    Ok(std::str::from_utf8(&config.bytes)?.parse()?)
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
        b"corium = present; corium { root = old; child = keep; }\nkernel.console = ttyS0\n"
            .to_vec(),
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
