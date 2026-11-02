// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    document::YamlDocument,
    planner::{Directive, Plan},
};
fn parse(text: &str) -> Result<Plan, conglobate::planner::PlanError> {
    Plan::try_from(YamlDocument::from(text))
}
#[test]
fn creation_exec_and_component_selection_are_accepted() {
    let plan = parse("image: rootfs\nstages: {rootfs: {use: registry.test/tools:1, steps: [{exec: {command: install, network: true, timeout: 5m}}]}}").unwrap();
    assert_eq!(plan.order[0].as_ref(), "rootfs");
    let Directive::Exec(exec) = &plan.stages.values().next().unwrap().derive[0] else {
        panic!()
    };
    assert!(exec.network);
    assert_eq!(exec.timeout.as_secs(), 300);
    assert!(parse("image: {root: rootfs, boot: {extract: {from: firmware, path: /boot.pmi}}}\nstages: {rootfs: {extend: registry.test/root:1}, firmware: {extend: rootfs}}").is_ok());
}
#[test]
fn old_vocabulary_and_ambiguous_shapes_are_rejected() {
    for text in [
        "{}",
        "image: {}",
        "image: missing",
        "bootstrap: {}",
        "carapace: {}",
        "firmware: {}",
        "transform: []",
        "platform: {}",
        "lock: {}",
        "image: registry.test/base:1\ntasks: {}",
        "image: null",
        "image: registry.test/base:1\nstages: null",
        "image: x\nimage: x\nstages: {}",
        "image: x\nstages: {x: {extend: a, extend: b}}",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
    for fields in [
        "using: registry.test/base:1",
        "extending: registry.test/base:1",
        "extend: null",
        "extend: registry.test/base:1, steps: null",
        "extend: registry.test/base:1, format: {}",
        "extend: registry.test/base:1, steps: [{exec: echo}]",
        "extend: registry.test/base:1, steps: [{exec: {command: echo, network: null}}]",
        "extend: registry.test/base:1, steps: [{exec: {command: echo, network: yes}}]",
        "extend: registry.test/base:1, steps: [{exec: {command: echo}, copy: {path: input, into: /x}}]",
        "extend: registry.test/base:1, steps: [{copy: {path: input, into: /x, owner: root}}]",
        "extend: registry.test/base:1, steps: [{copy: {from: registry.test/remote:1, path: input, into: /x}}]",
    ] {
        assert!(
            parse(&format!("image: x\nstages: {{x: {{{fields}}}}}")).is_err(),
            "{fields}"
        );
    }
}
#[test]
fn empty_steps_and_unrestricted_command_strings_are_accepted() {
    for steps in [
        "[]",
        "[{exec: {command: ''}}]",
        "[{exec: {command: ' ', timeout: 0s}}]",
    ] {
        assert!(
            parse(&format!(
                "image: x\nstages: {{x: {{extend: registry.test/base:1, steps: {steps}}}}}"
            ))
            .is_ok()
        );
    }
}
#[test]
fn conversion_checks_mounts_formats_and_unused_declarations() {
    for mounts in [
        "{'/': tmpfs}",
        "{/tmp: tmpfs, 'tmp/.': tmpfs}",
        "{dev: tmpfs}",
        "{/proc/hidden: tmpfs}",
        "{'./sys/': esp}",
    ] {
        assert!(parse(&format!("image: registry.test/base:1\nstages: {{unused: {{extend: registry.test/base:1, mounts: {mounts}}}}}")).is_err());
    }
    for format in [
        "kind: ext4",
        "fs: unknown",
        "size: 4096KiB",
        "fs: ext4, size: 15MiB",
        "label: null",
        "fs: ext4, label: abcdefghijklmnopq",
        "esp: 256MiB",
    ] {
        assert!(
            parse(&format!(
                "image: x\nstages: {{x: {{use: registry.test/tools:1, format: {{{format}}}}}}}"
            ))
            .is_err(),
            "{format}"
        );
    }
    assert!(
        parse("image: x\nstages: {x: {use: registry.test/tools:1, mounts: {/efi: esp}}}").is_err()
    );
    assert!(parse("image: x\nstages: {x: {use: registry.test/tools:1, format: {esp: 512MiB}, mounts: {/efi: esp, /tmp: tmpfs}}}").is_ok());
}
#[test]
fn graph_orders_copies_and_rejects_unknown_sources_and_unused_cycles() {
    let plan = parse("image: result\nstages:\n  orphan: {extend: missing.registry/test:1}\n  result: {extend: base, steps: [{copy: {from: producer, path: /out, into: /payload}}]}\n  producer: {extend: base}\n  base: {extend: registry.test/base:1}").unwrap();
    assert_eq!(
        plan.order
            .iter()
            .map(|name| name.as_ref())
            .collect::<Vec<_>>(),
        ["base", "producer", "result"]
    );
    for stages in [
        "{x: {extend: missing}}",
        "{x: {extend: y}, y: {extend: x}}",
        "{x: {extend: registry.test/base:1, steps: [{copy: {from: missing, path: x, into: /x}}]}}",
    ] {
        assert!(parse(&format!("image: registry.test/base:1\nstages: {stages}")).is_err());
    }
}
#[test]
fn shipped_staged_plans_use_the_runtime_contract() {
    for text in [
        include_str!("../../image/conglobate.plan.yaml"),
        include_str!("../distro/alpine/plan.yaml"),
        include_str!("../distro/debian/plan.yaml"),
        include_str!("../distro/fedora/plan.yaml"),
        include_str!("../distro/nixos/plan.yaml"),
        include_str!("../distro/firmware/plan.yaml"),
        include_str!("../distro/pipeline/plan.yaml"),
        include_str!("../distro/stage-mounts/plan.yaml"),
        include_str!("../distro/stage-mounts/nonzero.yaml"),
        include_str!("../distro/stage-mounts/timeout.yaml"),
        include_str!("../distro/pipeline/imported-boot.yaml"),
        include_str!("../distro/pipeline/boot-only.yaml"),
        include_str!("../distro/pipeline/extend.yaml"),
        include_str!("../distro/pipeline/copy-missing.yaml"),
        include_str!("../distro/confinement/plan.yaml"),
        include_str!("../distro/network/plan.yaml"),
    ] {
        parse(text).unwrap();
    }
}
