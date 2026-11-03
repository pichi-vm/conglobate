// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::plan::{
    Directive, FilesystemSize, ImageRef, Key, Plan, Stage, StageRef, Value as ConfigValue,
};
use serde_json::{Value, json};

fn decode(value: Value) -> Plan {
    serde_json::from_value(value).unwrap()
}

#[test]
fn all_boot_forms_have_closed_shapes_and_round_trip() {
    for boot in [
        json!("firmware"),
        json!({"inherit":"firmware"}),
        json!({"inherit":"firmware", "define":{}}),
        json!({"inherit":"firmware", "define":{"corium.root":""}}),
        json!({"inherit":"firmware", "extend":{"kernel.debug":null}}),
        json!({"extract":{"from":"builder", "path":"/image.pmi"}}),
        json!({"extract":{"from":"registry.test/builder:1", "path":"/image.pmi"}, "define":{"corium.root":""}}),
    ] {
        let input = json!({"version":1,"image":{"boot":boot}});
        let plan = decode(input.clone());
        assert_eq!(serde_json::to_value(plan).unwrap(), input);
    }
    for boot in [
        json!({"inherit":"firmware", "define":{}, "extend":{}}),
        json!({"inherit":"firmware", "extract":{"from":"builder", "path":"/image.pmi"}}),
        json!({"extract":{"path":"/image.pmi"}}),
        json!({"extract":{"from":"builder", "path":"/image.pmi"}, "extend":{}}),
        json!({"inherit":"firmware", "define":{"kernel.quiet":null}}),
        json!({"from":"firmware", "path":"/image.pmi"}),
        json!({"inherit":"firmware", "typo":true}),
    ] {
        assert!(
            serde_json::from_value::<Plan>(json!({"version":1,"image":{"boot":boot}})).is_err()
        );
    }
    assert!(
        serde_json::from_value::<Plan>(
            json!({"version":1,"image":{"boot":"firmware", "bootconfig":{}}})
        )
        .is_err()
    );
}

#[test]
fn extraction_empty_definition_normalizes_to_omission_but_inherited_clear_remains() {
    let mut input =
        json!({"version":1,"image":{"boot":{"extract":{"from":"builder", "path":"/image.pmi"}}}});
    let absent = decode(input.clone());
    input["image"]["boot"]["define"] = json!({});
    let empty = decode(input);
    assert_eq!(absent, empty);
    assert!(
        serde_json::to_value(empty).unwrap()["image"]["boot"]
            .get("define")
            .is_none()
    );
    let clear = decode(json!({"version":1,"image":{"boot":{"inherit":"firmware", "define":{}}}}));
    assert_eq!(
        serde_json::to_value(clear).unwrap()["image"]["boot"]["define"],
        json!({})
    );
}

#[test]
fn default_format_and_empty_steps_do_not_need_authored_fields() {
    let plan = decode(
        json!({"version":1,"stages":{"root":{"use":"registry.test/tools:1"}},"image":"root"}),
    );
    let Stage::Create { format, steps, .. } = &plan.stages.values().next().unwrap() else {
        panic!()
    };
    assert_eq!(format.filesystem, conglobate::plan::Filesystem::Ext4);
    assert_eq!(format.size.bytes(), 256 << 30);
    assert!(steps.is_empty());
    let normalized = serde_json::to_value(&plan).unwrap();
    assert!(normalized["stages"]["root"].get("steps").is_none());
    assert_eq!(decode(normalized), plan);
    decode(
        json!({"version":1,"stages":{"root":{"use":"tools", "format":{"fs":"ext4", "size":"512MiB", "label":"root", "esp":"512MiB"}}}, "image":"root"}),
    );
    for fs in ["xfs", "btrfs"] {
        assert!(
            serde_json::from_value::<Plan>(
                json!({"version":1,"stages":{"root":{"use":"tools", "format":{"fs":fs}}}, "image":"root"})
            )
            .is_err()
        );
    }
}

#[test]
fn stage_and_directive_shapes_reject_mixed_or_old_vocabulary() {
    for stage in [
        json!({"using":"tools"}),
        json!({"extending":"base"}),
        json!({"use":"tools", "extend":"base"}),
        json!({"extend":"base", "format":{}}),
        json!({"extend":"base", "mount":{}}),
        json!({"extend":"base", "derive":[]}),
        json!({"extend":"base", "steps":[{"exec":"true"}]}),
        json!({"extend":"base", "steps":[{"exec":{"command":"true"}, "network":true}]}),
        json!({"extend":"base", "steps":[{"exec":{"command":"true"}, "copy":{"path":"x", "into":"x"}}]}),
        json!({"extend":"base", "steps":[{"copy":{"from":"registry.test/root:1", "path":"x", "into":"x"}}]}),
        json!({"use":"tools", "format":{"fs":"ext4", "inodes":123}}),
    ] {
        assert!(
            serde_json::from_value::<Plan>(
                json!({"version":1,"stages":{"root":stage}, "image":"root"})
            )
            .is_err()
        );
    }
}

#[test]
fn durations_use_human_readable_strings_and_commands_are_plain_strings() {
    for (value, seconds) in [
        (json!({"command":" "}), 300),
        (json!({"command":"a\u{0}b", "timeout":"1h 30m"}), 5400),
    ] {
        let directive: Directive = serde_json::from_value(json!({"exec":value})).unwrap();
        let Directive::Exec { timeout, .. } = directive else {
            panic!()
        };
        assert_eq!(timeout.as_secs(), seconds);
        assert!(serde_json::to_value(directive).unwrap()["exec"]["timeout"].is_string());
    }
}

#[test]
fn scalar_constructors_and_serde_enforce_the_same_invariants() {
    for key in ["", ".root", "corium..root", "a=1", "a b", "a/child"] {
        assert!(Key::new(key.into()).is_none());
        assert!(serde_json::from_value::<Key>(json!(key)).is_err());
    }
    for size in [
        "1B",
        "4096KiB",
        "0MiB",
        "1.5GiB",
        "4294967296MiB",
        "4096TiB",
    ] {
        assert!(serde_json::from_value::<FilesystemSize>(json!(size)).is_err());
    }
    assert_eq!(std::mem::size_of::<FilesystemSize>(), 4);
    assert!(FilesystemSize::new(0).is_none());
    for (size, mib) in [("1MiB", 1), ("256GiB", 256 * 1024), ("1TiB", 1024 * 1024)] {
        assert_eq!(
            serde_json::from_value::<FilesystemSize>(json!(size)).unwrap(),
            FilesystemSize::new(mib).unwrap()
        );
    }
    for path in ["", "../escape", "/a/../b", "a\u{0}b"] {
        assert!(conglobate::plan::ImagePath::new(path.into()).is_none());
    }
}

#[test]
fn local_and_remote_reference_grammars_do_not_overlap() {
    for name in ["rootfs", "base-image", "firmware_1"] {
        assert!(matches!(
            serde_json::from_value::<ImageRef>(json!(name)).unwrap(),
            ImageRef::Local(_)
        ));
    }
    for name in [
        "localhost:5000/base:1",
        "registry.example.com/team/base:1",
        "[::1]:5000/base:1",
    ] {
        assert!(matches!(
            serde_json::from_value::<ImageRef>(json!(name)).unwrap(),
            ImageRef::Remote(_)
        ));
        assert!(StageRef::new(name.into()).is_none());
    }
    for name in [
        "team/base:1",
        "registry.example.com/a/../b",
        "https://registry.example.com/base",
        "registry.example.com:0/base",
    ] {
        assert!(serde_json::from_value::<ImageRef>(json!(name)).is_err());
    }
}

#[test]
fn bootconfig_preserves_empty_bare_and_list_values_with_deletion_only_in_extend() {
    for value in [json!(""), json!([]), json!(["ttyS0", "hvc0"])] {
        let decoded: ConfigValue = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    }
    for value in [
        Value::Null,
        json!(true),
        json!(10),
        json!([1]),
        json!({"nested":"unsupported"}),
    ] {
        assert!(serde_json::from_value::<ConfigValue>(value).is_err());
    }
}

#[cfg(feature = "cli")]
#[test]
fn yaml_example_and_scalar_types_use_the_same_contract() {
    let plan: Plan = serde_saphyr::from_str(include_str!("fixtures/staged-plan.yaml")).unwrap();
    assert_eq!(plan.stages.len(), 2);
    for value in ["true", "10", "[1]", "[true]"] {
        assert!(
            serde_saphyr::from_str::<ConfigValue>(value).is_err(),
            "{value}"
        );
    }
}

#[test]
fn string_wrappers_share_borrowed_lookup_and_display_traits() {
    use std::{
        borrow::Borrow,
        collections::BTreeSet,
        fmt::{Debug, Display},
        hash::Hash,
    };
    fn check<
        T: Clone
            + Debug
            + Display
            + AsRef<str>
            + Borrow<str>
            + std::ops::Deref<Target = str>
            + Into<String>
            + Ord
            + Hash,
    >(
        value: T,
    ) {
        let text = value.to_string();
        assert_eq!(value.as_ref(), text);
        assert_eq!(value.len(), text.len());
        assert_eq!(Into::<String>::into(value.clone()), text);
        let values = BTreeSet::from([value]);
        assert!(values.contains(text.as_str()));
    }
    check(StageRef::new("rootfs".into()).unwrap());
    check(conglobate::plan::ImagePath::new("/mnt/etc".into()).unwrap());
    check(Key::new("corium.root".into()).unwrap());
}

#[test]
fn remote_retains_components_without_registry_or_selector_defaults() {
    use conglobate::{artifact::ReferenceKind, plan::Remote};
    let remote = Remote::new("registry.example:5000/team/root:v1".into()).unwrap();
    assert_eq!(remote.registry(), "registry.example:5000");
    assert_eq!(remote.repository(), "team/root");
    assert_eq!(remote.selector(), &ReferenceKind::Tag("v1".into()));
    assert_eq!(remote.to_string(), "registry.example:5000/team/root:v1");
    let text = format!("[::1]:5000/team/root@sha256:{}", "ab".repeat(32));
    let remote: Remote = serde_json::from_value(json!(text)).unwrap();
    assert!(matches!(remote.selector(), ReferenceKind::Digest(_)));
    assert_eq!(serde_json::to_value(remote).unwrap(), json!(text));
    for text in [
        "alpine",
        "team/root:v1",
        "registry.example/root",
        "localhost:5000/root",
        "registry.example/root:",
    ] {
        assert!(Remote::new(text.into()).is_none(), "{text}");
    }
}

#[test]
fn plans_require_an_explicit_supported_version() {
    for input in [
        json!({"image":"root"}),
        json!({"version":2,"image":"root"}),
        json!({"version":null,"image":"root"}),
    ] {
        assert!(serde_json::from_value::<Plan>(input).is_err());
    }
}
