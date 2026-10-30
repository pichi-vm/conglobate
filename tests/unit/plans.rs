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
fn locked_filesystem_exec_and_image_sketches_are_accepted() {
    let plan = parse("image: rootfs\ntasks:\n  rootfs:\n    kind: bootstrap\n    from: registry.example.com/fedora:43\n    filesystem:\n      kind: ext4\n      size: 4GiB\n      label: rootfs\n    exec:\n      - run: dnf install -y nginx\n        network: true\n        timeout: 5m\n").unwrap();
    assert_eq!(plan.order()[0].as_ref(), "rootfs");
    let Directive::Run(run) = &plan.tasks().values().next().unwrap().directives()[0] else {
        panic!()
    };
    assert!(run.network());
    assert_eq!(run.timeout().duration().as_secs(), 300);
    assert!(parse("image: {root: rootfs, boot: firmware}\ntasks:\n  rootfs: {kind: extend, from: registry.example.com/root:1}\n  firmware: {kind: build, from: rootfs, into: {pmi: /boot.pmi, dtb: /boot.dtb}}\n").is_ok());
}
#[test]
fn reject_old_pipeline_and_unlocked_kind_vocabulary() {
    for text in [
        "{}",
        "image: x",
        "bootstrap: {}",
        "carapace: {}",
        "firmware: {}",
        "transform: []",
        "platform: {}",
        "lock: {}",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
    for kind in ["import", "platform", "export", "make", "produce"] {
        assert!(
            parse(&format!(
                "image: x\ntasks: {{x: {{kind: {kind}, from: registry.test/tool:1}}}}"
            ))
            .is_err()
        );
    }
}
#[test]
fn reject_unknown_duplicates_nulls_and_wrong_shapes_at_every_level() {
    let good = "image: x\ntasks: {x: {kind: extend, from: registry.test/base:1}}";
    assert!(parse(good).is_ok());
    for field in [
        "image: null",
        "tasks: null",
        "image: x\nimage: x\ntasks: {}",
        "image: x\ntasks: {x: {kind: extend, kind: build, from: registry.test/base:1}}",
    ] {
        assert!(parse(field).is_err());
    }
    for fields in [
        "from: null",
        "from: registry.test/base:1, exec: null",
        "from: registry.test/base:1, derive: []",
        "from: registry.test/base:1, filesystem: {kind: ext4, size: 16MiB}",
        "from: registry.test/base:1, exec: [{run: echo, network: null}]",
        "from: registry.test/base:1, exec: [{run: echo, network: yes}]",
        "from: registry.test/base:1, exec: [{run: echo, timeout: 0s}]",
        "from: registry.test/base:1, exec: [{run: echo, copy: {from: input, into: /x}}]",
        "from: registry.test/base:1, exec: [{copy: {from: input, into: /x, owner: root}}]",
        "from: registry.test/base:1, exec: [{copy: {from: a:b, into: /x}}]",
        "from: registry.test/base:1, exec: [{copy: {from: '*', into: /x}}]",
    ] {
        assert!(
            parse(&format!(
                "image: x\ntasks: {{x: {{kind: extend, {fields}}}}}"
            ))
            .is_err(),
            "{fields}"
        );
    }
    for fs in [
        "type: ext4, size: 16MiB",
        "kind: xfs, size: 16MiB",
        "kind: ext4, size: 4G",
        "kind: ext4, size: 16MiB, properties: {}",
        "kind: ext4, size: 16MiB, label: null",
        "kind: ext4, size: 15MiB",
        "kind: ext4, size: 16MiB, label: abcdefghijklmnopq",
    ] {
        assert!(parse(&format!("image: x\ntasks: {{x: {{kind: bootstrap, from: registry.test/tool:1, filesystem: {{{fs}}}}}}}")).is_err(),"{fs}");
    }
}
#[test]
fn graph_checks_missing_inputs_exports_cycles_and_reachable_closure() {
    let plan = parse("image: result\ntasks:\n  orphan: {kind: build, from: missing.registry/test:1, into: {ignored: /x}, exec: [{run: 'false', network: true}]}\n  result: {kind: extend, from: base, exec: [{copy: {from: payload, into: /payload}}]}\n  producer: {kind: build, from: base, into: {payload: /out}}\n  base: {kind: extend, from: registry.test/base:1}\n").unwrap();
    assert_eq!(
        plan.order().iter().map(|n| n.as_ref()).collect::<Vec<_>>(),
        ["base", "producer", "result"]
    );
    for text in [
        "image: x\ntasks: {x: {kind: extend, from: missing}}",
        "image: x\ntasks: {x: {kind: extend, from: y}, y: {kind: extend, from: x}}",
        "image: x\ntasks: {x: {kind: extend, from: registry.test/base:1, exec: [{copy: {from: payload, into: /x}}]}, y: {kind: build, from: x, into: {payload: /x}}}",
        "image: x\ntasks: {x: {kind: build, from: registry.test/base:1, into: {payload: /x}}}",
        "image: {root: x, boot: x}\ntasks: {x: {kind: bootstrap, from: registry.test/base:1, filesystem: {kind: ext4, size: 16MiB}}}",
        "image: registry.test/base:1\ntasks: {}",
        "image: x\ntasks: {x: {kind: extend, from: y}, y: {kind: build, from: registry.test/base:1, into: {pmi: /a, dtb: /b}}}",
    ] {
        assert!(parse(text).is_err(), "{text}");
    }
    assert!(parse("image: boot\ntasks: {boot: {kind: build, from: registry.test/base:1, into: {pmi: /a, dtb: /b}}}").is_ok());
}
#[test]
fn defaults_and_remote_qualification_are_strict() {
    use crate::plan::schema::{Local, Reference, Remote};
    for remote in [
        "registry.example.com/fedora:43",
        "localhost:5000/tool:1",
        "registry:5000/tool:1",
        "[::1]:5000/tool:1",
        "ghcr.io/pichi-vm/fedora",
        "localhost/tool@sha256:0000000000000000000000000000000000000000000000000000000000000000",
    ] {
        assert!(remote.parse::<Remote>().is_ok(), "{remote}");
        assert!(remote.parse::<Local>().is_err());
    }
    for bad in [
        "alpine",
        "alpine:latest",
        "user/image:tag",
        "./task",
        "registry.test/../tool",
        "registry.test/tool:bad:tag",
        "registry.test:bad/tool",
        "bad host.test/tool",
        "registry.test/tool@sha256:00",
    ] {
        assert!(bad.parse::<Remote>().is_err(), "{bad}");
    }
    let local: Reference = serde_saphyr::from_str("base").unwrap();
    assert!(matches!(local, Reference::Local(_)));
    let plan = parse(
        "image: x\ntasks: {x: {kind: extend, from: registry.test/base:1, exec: [{run: 'true'}]}}",
    )
    .unwrap();
    let Directive::Run(run) = &plan.tasks().values().next().unwrap().directives()[0] else {
        panic!()
    };
    assert!(!run.network());
    assert_eq!(run.timeout().duration().as_secs(), 300);
}
