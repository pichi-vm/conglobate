// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::{
    document::YamlDocument,
    lock::ImageLock,
    plan::{BuildPlan, InputRole, Stage},
    recipe::Recipe,
};
#[derive(Debug)]
struct Fixture;
impl Fixture {
    fn plan(recipe: &str) -> Result<BuildPlan, conglobate::lock::LockError> {
        let pins = format!(
            "version: 1\nimages:\n  base: {{index: sha256:{}}}\n  tools: {{index: sha256:{}}}\n",
            "00".repeat(32),
            "11".repeat(32)
        );
        let lock = ImageLock::try_from(YamlDocument::from(pins.as_str())).unwrap();
        BuildPlan::new(Recipe::try_from(YamlDocument::from(recipe)).unwrap(), &lock)
    }
}
#[test]
fn pipeline_order_and_network_receipt_order_ignore_yaml_key_order() {
    let plan=Fixture::plan("firmware:\n  from: tools\n  into: {pmi: /pmi, dtb: /dtb}\n  derive: [{run: build, network: true}]\ncarapace:\n  from: base\n  derive: [{copy: {from: input, into: /build}}, {run: install, network: true}]\n").unwrap();
    assert_eq!(
        plan.stages(),
        &[Stage::Carapace, Stage::Firmware, Stage::Transform]
    );
    assert_eq!(
        plan.network_steps()
            .iter()
            .map(|s| (s.stage(), s.index()))
            .collect::<Vec<_>>(),
        vec![(Stage::Carapace, 1), (Stage::Firmware, 0)]
    );
    assert_eq!(plan.inputs().len(), 2);
    assert!(plan.inputs().iter().all(|i| i.execution_required()));
}
#[test]
fn shared_reference_retains_roles_without_conflating_output_ancestry() {
    let plan =
        Fixture::plan("carapace: {from: base}\nfirmware: {from: base, into: {pmi: /a, dtb: /b}}")
            .unwrap();
    assert_eq!(plan.inputs().len(), 1);
    assert!(plan.inputs()[0].roles().contains(&InputRole::CarapaceBase));
    assert!(plan.inputs()[0].roles().contains(&InputRole::FirmwareTools));
    assert!(plan.inputs()[0].execution_required());
}
#[test]
fn pass_through_base_does_not_require_execution_but_implicit_tools_do() {
    let pass = Fixture::plan("carapace: {from: base}\ntransform: []").unwrap();
    assert!(!pass.inputs()[0].execution_required());
    let firmware =
        Fixture::plan("carapace: {from: base}\nfirmware: {into: {pmi: /a, dtb: /b}}").unwrap();
    assert!(firmware.inputs()[0].execution_required());
    assert!(Fixture::plan("carapace: {from: missing}").is_err());
}
