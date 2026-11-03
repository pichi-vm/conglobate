// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{document::YamlDocument, graph::BuildGraph, lock::ImageLock, planner::Plan};
fn graph(plan: &str) -> Result<BuildGraph, conglobate::lock::LockError> {
    let pins = format!(
        "version: 1\nimages:\n  registry.test/base:1: sha256:{}\n  registry.test/tools:1: sha256:{}\n",
        "00".repeat(32),
        "11".repeat(32)
    );
    BuildGraph::new(
        Plan::try_from(YamlDocument::from(plan)).unwrap(),
        &ImageLock::try_from(YamlDocument::from(pins.as_str())).unwrap(),
    )
}
#[test]
fn only_selected_closure_requires_pins_execution_or_network() {
    let plan = "version: 1\nimage: registry.test/base:1\nstages:\n  orphan: {extend: missing.registry/tool:1, steps: [exec: {command: never, network: true}]}\n";
    let graph = graph(plan).unwrap();
    assert_eq!(graph.inputs().len(), 1);
    assert!(!graph.inputs()[0].root_required());
    assert!(graph.network_steps().is_empty());
    assert!(!graph.execution_required());
}
#[test]
fn repeated_sources_are_locked_once_and_network_steps_keep_stage_identity() {
    let graph = graph("version: 1\nimage: {root: base, boot: {extract: {from: boot, path: /a}}}\nstages:\n  base: {extend: registry.test/base:1, steps: [copy: {path: input, into: /x}, exec: {command: install, network: true}]}\n  boot: {extend: registry.test/base:1, steps: [exec: {command: compile, network: true}]}\n").unwrap();
    assert_eq!(graph.inputs().len(), 1);
    assert!(graph.inputs()[0].root_required());
    assert_eq!(
        graph
            .network_steps()
            .iter()
            .map(|(name, index)| (name.as_ref(), *index))
            .collect::<Vec<_>>(),
        [("base", 1), ("boot", 0)]
    );
}
#[test]
fn local_inputs_are_not_lock_entries_and_missing_remote_pin_fails() {
    assert!(graph("version: 1\nimage: child\nstages: {child: {extend: base, steps: [exec: {command: child}]}, base: {extend: registry.test/base:1, steps: [exec: {command: base}]}}\n").is_ok());
    assert!(graph("version: 1\nimage: registry.test/missing:1").is_err());
    assert!(
        graph("version: 1\nimage: {root: registry.test/missing:1, boot: registry.test/base:1}")
            .is_err()
    );
}
#[test]
fn direct_component_selectors_are_locked_without_root_activation() {
    let selected =
        graph("version: 1\nimage: {root: registry.test/base:1, boot: registry.test/tools:1}")
            .unwrap();
    assert_eq!(selected.inputs().len(), 2);
    assert!(selected.inputs().iter().all(|input| !input.root_required()));
    assert!(!selected.execution_required());
    let file = graph(
        "version: 1\nimage: {boot: {extract: {from: registry.test/tools:1, path: /boot.pmi}}}",
    )
    .unwrap();
    assert!(file.execution_required());
    assert!(!file.plan().commands_required());
}
