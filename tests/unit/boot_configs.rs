// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::contract::{artifact::BootConfig, recipe};

#[test]
fn values_are_literal_and_do_not_expand_shell_or_build_variables() {
    for text in [
        "$${CARAPACE} ${CARAPACE} $$",
        "${word}:$1:$2:$99:${missing}",
        "$(touch /outside)",
        "a; b",
    ] {
        let overlay: recipe::BootConfig =
            serde_json::from_value(serde_json::json!({"values":{"kernel.argument":text}})).unwrap();
        let config = overlay.apply(&BootConfig::default()).unwrap();
        let decoded: BootConfig = config.as_ref().parse().unwrap();
        assert_eq!(decoded.values()["kernel.argument"], [text]);
    }
}
#[test]
fn overlays_distinguish_bare_empty_list_and_exact_deletion() {
    let base: BootConfig = "corium { root = old; child = keep; }\nkernel.console = ttyS0\n"
        .parse()
        .unwrap();
    let overlay:recipe::BootConfig=serde_json::from_value(serde_json::json!({"values":{"corium":null,"corium.root":"","kernel.console":["ttyS0","hvc0"],"kernel.quiet":[]}})).unwrap();
    let result = overlay.apply(&base).unwrap();
    assert_eq!(result.values()["corium.root"], [""]);
    assert_eq!(result.values()["corium.child"], ["keep"]);
    assert_eq!(result.values()["kernel.console"], ["ttyS0", "hvc0"]);
    assert!(result.values()["kernel.quiet"].is_empty());
}
#[test]
fn invalid_keys_and_unrepresentable_values_fail_before_execution() {
    for (key, value) in [
        ("kernel.bad key", "text"),
        ("kernel.arg", "nul\0"),
        ("kernel.arg", "both '\" quotes; delimiter"),
    ] {
        let overlay: recipe::BootConfig =
            serde_json::from_value(serde_json::json!({"values":{key:value}})).unwrap();
        assert!(
            overlay.apply(&BootConfig::default()).is_err(),
            "{key}: {value:?}"
        );
    }
}
