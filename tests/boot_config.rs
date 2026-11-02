// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    artifact::BootConfig,
    recipe::{BootConfig as Overlay, BootConfigValue},
};
use std::collections::BTreeMap;

#[test]
fn native_syntax_preserves_values_and_override_semantics() {
    let config: BootConfig = r#"
        # Quotes do not interpret backslashes.
        kernel { quiet; console = ttyS0, "hvc0"; }
        corium.root = "";
        parent = old; parent { child = 'with # ; , } characters'; }
        parent := new; kernel.console += tty0;
        bare; bare; backslash = "a\b";
        array = , , last;
    "#
    .parse()
    .unwrap();
    assert_eq!(config.values()["kernel.quiet"], Vec::<String>::new());
    assert_eq!(config.values()["corium.root"], [""]);
    assert_eq!(config.values()["kernel.console"], ["ttyS0", "hvc0", "tty0"]);
    assert_eq!(config.values()["parent"], ["new"]);
    assert_eq!(config.values()["parent.child"], ["with # ; , } characters"]);
    assert_eq!(config.values()["backslash"], ["a\\b"]);
    assert_eq!(config.values()["array"], ["", "", "last"]);
    assert_eq!(config, config.to_string().parse().unwrap());
}

#[test]
fn overlays_delete_exact_keys_and_replace_whole_arrays() {
    let base: BootConfig = "parent=old;parent.child=kept;corium.root=hash;a=one,two;"
        .parse()
        .unwrap();
    let overlay = Overlay {
        from: None,
        values: BTreeMap::from([
            ("parent".into(), BootConfigValue::Delete),
            ("corium.root".into(), BootConfigValue::Delete),
            ("a".into(), BootConfigValue::List(vec!["new".into()])),
            ("bare".into(), BootConfigValue::List(vec![])),
            ("empty".into(), BootConfigValue::String(String::new())),
        ]),
    };
    let result = overlay.apply(&base).unwrap();
    assert!(!result.values().contains_key("parent"));
    assert!(!result.values().contains_key("corium.root"));
    assert_eq!(result.values()["parent.child"], ["kept"]);
    assert_eq!(result.values()["a"], ["new"]);
    assert_eq!(result.values()["bare"], Vec::<String>::new());
    assert_eq!(result.values()["empty"], [""]);
    assert_eq!(base.values()["parent"], ["old"]);
    assert_eq!(
        "parent;parent.child=kept;"
            .parse::<BootConfig>()
            .unwrap()
            .values()
            .len(),
        1
    );
}

#[test]
fn malformed_text_and_unrepresentable_values_fail() {
    for text in [
        "a",
        "# empty",
        "a=x;a=y;",
        "a..b=x;",
        ".a=x;",
        "a b=x;",
        "a+=x;a=y;",
        "a : = x;",
        "a { b=x;",
        "a=x;}",
        "a='x' trailing;",
        "a=\"unterminated",
        "a=é;",
        "a=\0;",
        "a=\x7f;",
    ] {
        assert!(text.parse::<BootConfig>().is_err(), "{text:?}");
    }
    for value in ["both '\";quotes", "'starts\"", "both '\"\n", "nul\0", "é"] {
        assert!(
            BootConfig::try_from(BTreeMap::from([("key".into(), vec![value.into()])])).is_err(),
            "{value:?}"
        );
    }
    for value in ["", "with spaces", "both'\"quotes", "a\\b", "a\nb", " tab\t"] {
        let config =
            BootConfig::try_from(BTreeMap::from([("key".into(), vec![value.into()])])).unwrap();
        assert_eq!(config, config.to_string().parse().unwrap());
    }
    // At EOF Linux keeps trailing unquoted whitespace.
    assert_eq!(
        "key=value  ".parse::<BootConfig>().unwrap().values()["key"],
        ["value  "]
    );
}

#[test]
fn linux_limits_and_native_reservation_framing_are_enforced() {
    for (key, valid) in [
        ("a".repeat(255), true),
        ("a".repeat(256), false),
        (vec!["a"; 16].join("."), true),
        (vec!["a"; 17].join("."), false),
    ] {
        assert_eq!(
            BootConfig::try_from(BTreeMap::from([(key, vec![])])).is_ok(),
            valid
        );
    }
    assert!(BootConfig::try_from(BTreeMap::from([("a".into(), vec!["".into(); 8192])])).is_err());
    assert!(
        format!("a={}", "x".repeat(BootConfig::MAX_SIZE))
            .parse::<BootConfig>()
            .is_err()
    );
    let config: BootConfig = "a=x;".parse().unwrap();
    let capacity = ((config.as_ref().len() + 4) & !3) as u64 + 20;
    config.check_capacity(capacity).unwrap();
    assert!(config.check_capacity(capacity - 4).is_err());
    assert!(config.check_capacity(capacity + 1).is_err());
    BootConfig::default().check_capacity(0).unwrap();
}

#[test]
#[ignore = "requires the Linux bootconfig utility; run explicitly for native format qualification"]
fn linux_utility_accepts_native_and_canonical_text_with_identical_values() {
    use std::{fs, process::Command};
    let directory =
        std::env::temp_dir().join(format!("conglobate-bootconfig-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let path = directory.join("config");
    for text in [
        "a; b=; c=x,y;",
        "a { b=x; c='space # quote'; }; a.b:=new; a.b+=last;",
        "a=x; a.child=y; a;",
        "a = x,\n# comment\ny,;",
        "a=\"multiline\nvalue\"; backslash='a\\b';",
        "a=x  ",
        "a={braces; b='a\"b'; c=\"a'b\";",
        "a{};",
    ] {
        let config: BootConfig = text.parse().unwrap();
        for input in [text, config.as_ref()] {
            fs::write(&path, input).unwrap();
            let output =
                Command::new(std::env::var_os("BOOTCONFIG").unwrap_or_else(|| "bootconfig".into()))
                    .arg(&path)
                    .output()
                    .expect("install the Linux bootconfig utility");
            assert!(
                output.status.success(),
                "{input:?}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let parsed: BootConfig = String::from_utf8(output.stdout).unwrap().parse().unwrap();
            assert_eq!(config, parsed, "{input:?}");
        }
    }
    for text in [
        "a",
        "a=x;a=y;",
        "a..b=x;",
        "a b=x;",
        "a : = x;",
        "a {b=x;",
        "a=x;}",
        "a='x' trailing;",
        "a=é;",
    ] {
        fs::write(&path, text).unwrap();
        let output =
            Command::new(std::env::var_os("BOOTCONFIG").unwrap_or_else(|| "bootconfig".into()))
                .arg(&path)
                .output()
                .unwrap();
        assert!(!output.status.success(), "{text:?}");
        assert!(text.parse::<BootConfig>().is_err(), "{text:?}");
    }
    // Replacement leaves old array nodes allocated in the Linux parser.
    for (count, valid) in [(4095, true), (4096, false)] {
        let text = format!("a={};a:={};", ",".repeat(count), ",".repeat(count));
        fs::write(&path, &text).unwrap();
        let output =
            Command::new(std::env::var_os("BOOTCONFIG").unwrap_or_else(|| "bootconfig".into()))
                .arg(&path)
                .output()
                .unwrap();
        assert_eq!(output.status.success(), valid);
        assert_eq!(text.parse::<BootConfig>().is_ok(), valid);
    }
    fs::remove_dir_all(directory).unwrap();
}
