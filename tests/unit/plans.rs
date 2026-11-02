// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    document::YamlDocument,
    plan::{Directive, Plan},
};
fn parse(text: &str) -> Result<Plan, conglobate::plan::PlanError> {
    Plan::try_from(YamlDocument::from(text))
}
#[test]
fn creation_exec_and_component_selection_are_accepted() {
    let plan = parse("image: rootfs\nstages:\n  rootfs:\n    using: registry.example.com/fedora:43\n    format: {kind: ext4, size: 4GiB, label: rootfs}\n    derive:\n      - exec: dnf install -y nginx\n        network: true\n        timeout: 5m\n").unwrap();
    assert_eq!(plan.order[0].as_ref(), "rootfs");
    let Directive::Exec(exec) = &plan.stages.values().next().unwrap().derive[0] else {
        panic!()
    };
    assert!(exec.network);
    assert_eq!(exec.timeout.duration().as_secs(), 300);
    assert!(parse("image: {root: rootfs, boot: {from: firmware, path: /boot.pmi}}\nstages:\n  rootfs: {extending: registry.test/root:1, derive: [{exec: install}]}\n  firmware: {extending: rootfs, derive: [{exec: compile}]}\n").is_ok());
}
#[test]
fn old_task_and_pipeline_vocabulary_is_rejected() {
    for text in [
        "{}",
        "image: missing",
        "bootstrap: {}",
        "carapace: {}",
        "firmware: {}",
        "transform: []",
        "platform: {}",
        "lock: {}",
        "image: registry.test/base:1\ntasks: {}",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
    for kind in [
        "bootstrap",
        "extend",
        "build",
        "import",
        "platform",
        "export",
        "make",
        "produce",
    ] {
        assert!(
            parse(&format!(
                "image: x\nstages: {{x: {{kind: {kind}, from: registry.test/tool:1}}}}"
            ))
            .is_err()
        );
    }
}
#[test]
fn unknown_duplicates_nulls_and_wrong_shapes_are_rejected() {
    assert!(
        parse("image: x\nstages: {x: {extending: registry.test/base:1, derive: [{exec: 'true'}]}}")
            .is_ok()
    );
    for text in [
        "image: null",
        "image: registry.test/base:1\nstages: null",
        "image: x\nimage: x\nstages: {}",
        "image: x\nstages: {x: {extending: a, extending: b, derive: [{exec: 'true'}]}}",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
    for fields in [
        "extending: null, derive: [{exec: 'true'}]",
        "extending: registry.test/base:1, derive: null",
        "extending: registry.test/base:1, derive: []",
        "extending: registry.test/base:1, format: {kind: ext4, size: 16MiB}, derive: [{exec: 'true'}]",
        "extending: registry.test/base:1, derive: [{exec: echo, network: null}]",
        "extending: registry.test/base:1, derive: [{exec: echo, network: yes}]",
        "extending: registry.test/base:1, derive: [{exec: echo, timeout: 0s}]",
        "extending: registry.test/base:1, derive: [{exec: echo, copy: {path: input, into: /x}}]",
        "extending: registry.test/base:1, derive: [{copy: {path: input, into: /x, owner: root}}]",
        "extending: registry.test/base:1, derive: [{copy: {from: registry.test/remote:1, path: input, into: /x}}]",
    ] {
        assert!(
            parse(&format!("image: x\nstages: {{x: {{{fields}}}}}")).is_err(),
            "{fields}"
        );
    }
    for format in [
        "type: ext4, size: 16MiB",
        "kind: xfs, size: 16MiB",
        "kind: ext4, size: 4G",
        "kind: ext4, size: 16MiB, properties: {}",
        "kind: ext4, size: 16MiB, label: null",
        "kind: ext4, size: 15MiB",
        "kind: ext4, size: 16MiB, label: abcdefghijklmnopq",
        "kind: ext4, size: 16MiB, esp: 256MiB",
    ] {
        assert!(parse(&format!("image: x\nstages: {{x: {{using: registry.test/tool:1, format: {{{format}}}, derive: [{{exec: 'true'}}]}}}}")).is_err(), "{format}");
    }
    assert!(parse("image: x\nstages: {x: {using: registry.test/tool:1, format: {kind: ext4, size: 16MiB}, mount: {/efi: esp}, derive: [{exec: 'true'}]}}").is_err());
}
#[test]
fn graph_checks_named_copies_cycles_and_selected_closure() {
    let plan = parse("image: result\nstages:\n  orphan: {extending: missing.registry/test:1, derive: [{exec: 'false', network: true}]}\n  result: {extending: base, derive: [{copy: {from: producer, path: /out, into: /payload}}]}\n  producer: {extending: base, derive: [{exec: produce}]}\n  base: {extending: registry.test/base:1, derive: [{exec: base}]}\n").unwrap();
    assert_eq!(
        plan.order
            .iter()
            .map(|name| name.as_ref())
            .collect::<Vec<_>>(),
        ["base", "producer", "result"]
    );
    for text in [
        "image: x\nstages: {x: {extending: missing, derive: [{exec: x}]}}",
        "image: x\nstages: {x: {extending: y, derive: [{exec: x}]}, y: {extending: x, derive: [{exec: y}]}}",
        "image: x\nstages: {x: {extending: registry.test/base:1, derive: [{copy: {from: y, path: payload, into: /x}}]}, y: {extending: x, derive: [{exec: y}]}}",
        "image: {bootconfig: {values: {corium.root: ''}}}",
        "image: {root: registry.test/base:1, bootconfig: {}}",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
    assert!(parse("image: registry.test/base:1").is_ok());
    assert!(parse("image: {boot: registry.test/base:1}").is_ok());
}
#[test]
fn defaults_and_remote_qualification_are_strict() {
    use crate::contract::recipe::{ImageRef, Remote, StageRef};
    for remote in [
        "registry.example.com/fedora:43",
        "localhost:5000/tool:1",
        "registry:5000/tool:1",
        "[::1]:5000/tool:1",
        "ghcr.io/pichi-vm/fedora",
        "localhost/tool@sha256:0000000000000000000000000000000000000000000000000000000000000000",
    ] {
        assert!(remote.parse::<Remote>().is_ok(), "{remote}");
        assert!(remote.parse::<StageRef>().is_err());
    }
    for bad in [
        "alpine",
        "alpine:latest",
        "user/image:tag",
        "./stage",
        "registry.test/../tool",
        "registry.test/tool:bad:tag",
        "registry.test:bad/tool",
        "bad host.test/tool",
        "registry.test/tool@sha256:00",
    ] {
        assert!(bad.parse::<Remote>().is_err(), "{bad}");
    }
    let local: ImageRef = serde_saphyr::from_str("base").unwrap();
    assert!(matches!(local, ImageRef::Local(_)));
    let plan =
        parse("image: x\nstages: {x: {extending: registry.test/base:1, derive: [{exec: 'true'}]}}")
            .unwrap();
    let Directive::Exec(exec) = &plan.stages.values().next().unwrap().derive[0] else {
        panic!()
    };
    assert!(!exec.network);
    assert_eq!(exec.timeout.duration().as_secs(), 300);
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
    for fields in [
        "derive: null",
        "derive: [{exec: 123}]",
        "derive: [{copy: {path: 123, into: /x}}]",
    ] {
        assert!(
            parse(&format!(
                "stages: {{root: {{extending: registry.test/base:1, {fields}}}}}\nimage: root"
            ))
            .is_err()
        );
    }
}
