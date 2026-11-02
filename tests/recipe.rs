// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::recipe::{BootConfigValue, ImageRef, Plan, StageRef};
use serde_json::{Value, json};

fn recipe(value: Value) -> Plan {
    serde_json::from_value(value).unwrap()
}

#[test]
fn all_component_combinations_and_remote_shortcut_round_trip() {
    for image in [
        json!("registry.example.com/base:latest"),
        json!({"root":"registry.example.com/base:latest"}),
        json!({"boot":"registry.example.com/firmware:latest"}),
        json!({"root":"registry.example.com/base:latest", "boot":"registry.example.com/firmware:latest"}),
        json!({"boot":"registry.example.com/firmware:latest", "bootconfig":{"values":{"corium.root":""}}}),
        json!({"root":"registry.example.com/base:latest", "boot":{"from":"registry.example.com/builder:latest", "path":"out/image.pmi"}, "bootconfig":"registry.example.com/config:latest"}),
    ] {
        let input = json!({"image":image});
        let plan = recipe(input.clone());
        assert!(plan.order().unwrap().is_empty());
        assert_eq!(serde_json::to_value(&plan).unwrap(), input);
    }
    for image in [
        json!({}),
        json!({"bootconfig":{}}),
        json!({"root":"registry.example.com/base:latest", "bootconfig":{}}),
    ] {
        assert!(recipe(json!({"image":image})).validate().is_err());
    }
}

#[test]
fn graph_orders_sources_and_copies_and_omits_unused_work() {
    let plan = recipe(json!({
        "stages": {
            "output": {"extending":"base", "derive":[{"copy":{"from":"producer", "path":"out", "into":"/usr/share"}}]},
            "producer": {"extending":"base", "derive":[{"exec":"mkdir -p /out"}]},
            "base": {"using":"registry.example.com/tools:1", "format":{"kind":"ext4", "size":"1GiB"}, "derive":[{"exec":"mkdir -p /mnt/etc"}]},
            "unused": {"extending":"registry.example.com/unused:1", "derive":[{"exec":"true"}]}
        },
        "image":"output"
    }));
    let order: Vec<_> = plan
        .order()
        .unwrap()
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(order, ["base", "producer", "output"]);
    assert_eq!(recipe(serde_json::to_value(&plan).unwrap()), plan);
}

#[test]
fn unused_cycles_and_unknown_copy_sources_are_still_invalid() {
    for stages in [
        json!({"a":{"extending":"b", "derive":[{"exec":"true"}]}, "b":{"extending":"a", "derive":[{"exec":"true"}]}}),
        json!({"a":{"extending":"registry.example.com/base:1", "derive":[{"copy":{"from":"missing", "path":"x", "into":"/x"}}]}}),
    ] {
        assert!(
            recipe(json!({"stages":stages, "image":"registry.example.com/base:1"}))
                .validate()
                .is_err()
        );
    }
    assert!(recipe(json!({"image":"missing"})).validate().is_err());
}

#[test]
fn derive_is_required_and_stages_have_closed_disjoint_shapes() {
    for stage in [
        json!({"extending":"registry.example.com/base:1"}),
        json!({"using":"registry.example.com/base:1", "derive":[{"exec":"true"}]}),
        json!({"extending":"registry.example.com/base:1", "using":"registry.example.com/base:1", "derive":[{"exec":"true"}]}),
        json!({"extending":"registry.example.com/base:1", "format":{"kind":"ext4","size":"1GiB"}, "derive":[{"exec":"true"}]}),
        json!({"extending":"registry.example.com/base:1", "derive":[{"run":"true"}]}),
        json!({"extending":"registry.example.com/base:1", "derive":[{"exec":"true", "typo":true}]}),
    ] {
        assert!(
            serde_json::from_value::<Plan>(json!({"stages":{"test":stage},"image":"test"}))
                .is_err()
        );
    }
    for derive in [
        json!([]),
        json!([{"exec":" "}]),
        json!([{"exec":"a\u{0}b"}]),
    ] {
        assert!(recipe(json!({"stages":{"test":{"extending":"registry.example.com/base:1", "derive":derive}}, "image":"test"})).validate().is_err());
    }
}

#[test]
fn copy_namespaces_and_mount_paths_preserve_confinement() {
    for path in ["../escape", "/a/../b", "a\u{0}b", ""] {
        assert!(serde_json::from_value::<Plan>(json!({"stages":{"test":{"extending":"registry.example.com/base:1", "derive":[{"copy":{"path":path,"into":"/x"}}]}}, "image":"test"})).is_err());
    }
    for mount in [
        json!({"/":"tmpfs"}),
        json!({"/tmp":"tmpfs", "tmp/.":"tmpfs"}),
        json!({"dev":"tmpfs"}),
        json!({"/proc/hidden":"tmpfs"}),
        json!({"./sys/":"esp"}),
    ] {
        assert!(recipe(json!({"stages":{"test":{"extending":"registry.example.com/base:1", "mount":mount, "derive":[{"exec":"true"}]}},"image":"test"})).validate().is_err());
    }
    let plan = recipe(
        json!({"stages":{"test":{"extending":"registry.example.com/base:1", "mount":{"/tmp":"tmpfs", "boot/efi":"esp"}, "derive":[{"copy":{"path":"files","into":"/etc"}}]}},"image":"test"}),
    );
    plan.validate().unwrap();
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
        assert!(name.parse::<StageRef>().is_err());
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
fn bootconfig_preserves_empty_bare_list_and_delete() {
    let values = [json!(""), json!([]), json!(["ttyS0", "hvc0"]), Value::Null];
    for value in values {
        let decoded: BootConfigValue = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    }
    for value in [
        json!(true),
        json!(10),
        json!([1]),
        json!({"nested":"unsupported"}),
    ] {
        assert!(serde_json::from_value::<BootConfigValue>(value).is_err());
    }
}

#[cfg(feature = "cli")]
#[test]
fn yaml_example_and_scalar_types_use_the_same_contract() {
    let text = include_str!("fixtures/staged-recipe.yaml");
    let plan: Plan = serde_saphyr::from_str(text).unwrap();
    plan.validate().unwrap();
    assert_eq!(plan.order().unwrap().len(), 2);
    for value in ["true", "10", "[1]", "[true]"] {
        assert!(
            serde_saphyr::from_str::<BootConfigValue>(value).is_err(),
            "{value}"
        );
    }
}
