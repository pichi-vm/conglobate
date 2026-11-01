// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{document::YamlDocument, graph::BuildGraph, lock::ImageLock, plan::Plan};
fn graph(plan: &str) -> Result<BuildGraph, conglobate::lock::LockError> {
    let pins = format!(
        "version: 1\nimages:\n  registry.test/base:1: {{index: sha256:{}}}\n  registry.test/tools:1: {{index: sha256:{}}}\n",
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
    let plan = "image: registry.test/base:1\nstages:\n  orphan: {extending: missing.registry/tool:1, derive: [{exec: never, network: true}]}";
    let graph = graph(plan).unwrap();
    assert_eq!(graph.inputs().len(), 1);
    assert!(!graph.inputs()[0].root_required());
    assert!(graph.network_steps().is_empty());
    assert!(!graph.execution_required());
}
#[test]
fn repeated_sources_are_locked_once_and_network_steps_keep_stage_identity() {
    let graph = graph("image: {root: base, boot: {from: boot, path: /a}}\nstages:\n  base: {extending: registry.test/base:1, derive: [{copy: {path: input, into: /x}}, {exec: install, network: true}]}\n  boot: {extending: registry.test/base:1, derive: [{exec: compile, network: true}]}\n").unwrap();
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
    assert!(graph("image: child\nstages: {child: {extending: base, derive: [{exec: child}]}, base: {extending: registry.test/base:1, derive: [{exec: base}]}}").is_ok());
    assert!(graph("image: registry.test/missing:1").is_err());
    assert!(
        graph("image: {boot: registry.test/base:1, bootconfig: registry.test/missing:1}").is_err()
    );
}
#[test]
fn direct_component_selectors_are_locked_without_root_activation() {
    let selected =
        graph("image: {boot: registry.test/tools:1, bootconfig: registry.test/base:1}").unwrap();
    assert_eq!(selected.inputs().len(), 2);
    assert!(selected.inputs().iter().all(|input| !input.root_required()));
    assert!(!selected.execution_required());
    let file = graph("image: {boot: {from: registry.test/tools:1, path: /boot.pmi}}").unwrap();
    assert!(file.execution_required());
    assert!(!file.plan().commands_required());
}
