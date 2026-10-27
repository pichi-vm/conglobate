// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::{
    document::YamlDocument,
    lock::ImageLock,
    plan::{BuildPlan, InputRole, Stage},
    recipe::Recipe,
};
fn plan(recipe: &str) -> Result<BuildPlan, conglobate::lock::LockError> {
    let pins = format!(
        "version: 1\nimages:\n  registry.test/base:1: {{index: sha256:{}}}\n  registry.test/tools:1: {{index: sha256:{}}}\n",
        "00".repeat(32),
        "11".repeat(32)
    );
    BuildPlan::new(
        Recipe::try_from(YamlDocument::from(recipe)).unwrap(),
        &ImageLock::try_from(YamlDocument::from(pins.as_str())).unwrap(),
    )
}
#[test]
fn only_selected_closure_requires_pins_execution_or_network() {
    let recipe = "image: base\ntasks:\n  base: {kind: extend, from: registry.test/base:1}\n  orphan: {kind: build, from: missing.registry/tool:1, into: {data: /x}, exec: [{run: never, network: true}]}";
    let plan = plan(recipe).unwrap();
    assert_eq!(plan.inputs().len(), 1);
    assert!(plan.network_steps().is_empty());
    assert!(!plan.execution_required());
}
#[test]
fn remote_tool_roles_do_not_imply_output_ancestry() {
    let plan = plan("image: {root: base, boot: boot}\ntasks:\n  base: {kind: extend, from: registry.test/base:1, exec: [{copy: {from: input, into: /x}}, {run: install, network: true}]}\n  boot: {kind: build, from: registry.test/base:1, exec: [{run: compile, network: true}], into: {pmi: /a, dtb: /b}}\n").unwrap();
    assert_eq!(plan.inputs().len(), 1);
    assert!(plan.inputs()[0].roles().contains(&InputRole::CarapaceBase));
    assert!(plan.inputs()[0].roles().contains(&InputRole::FirmwareTools));
    assert_eq!(
        plan.network_steps()
            .iter()
            .map(|s| (s.stage(), s.index()))
            .collect::<Vec<_>>(),
        [(Stage::Carapace, 1), (Stage::Firmware, 0)]
    );
}
#[test]
fn local_inputs_are_not_lock_entries_and_missing_remote_pin_fails() {
    assert!(plan("image: child\ntasks: {child: {kind: extend, from: base}, base: {kind: extend, from: registry.test/base:1}}").is_ok());
    assert!(plan("image: x\ntasks: {x: {kind: extend, from: registry.test/missing:1}}").is_err());
}
