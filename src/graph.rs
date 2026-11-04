// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    contract::plan::{ImageRef, StageRef},
    image::{ImageReference, IndexDigest},
    lock::{ImageLock, LockError},
    planner::{Directive, Plan},
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct ImageInput {
    reference: ImageReference,
    index: Option<IndexDigest>,
    root_required: bool,
}
impl ImageInput {
    pub(crate) fn reference(&self) -> &ImageReference {
        &self.reference
    }
    pub(crate) fn index(&self) -> Option<IndexDigest> {
        self.index
    }
    pub(crate) fn root_required(&self) -> bool {
        self.root_required
    }
}
#[derive(Debug)]
pub(crate) struct BuildGraph {
    plan: Plan,
    inputs: Vec<ImageInput>,
    network_steps: Vec<(StageRef, usize)>,
}
impl BuildGraph {
    pub(crate) fn new(plan: Plan, lock: &ImageLock) -> Result<Self, LockError> {
        let mut inputs = BTreeMap::new();
        for (remote, bootstrap) in plan.remote_sources() {
            let reference = ImageReference::from(remote);
            let index = if bootstrap {
                None
            } else {
                Some(lock.resolve(&reference)?)
            };
            let input = inputs.entry(reference.clone()).or_insert(ImageInput {
                index,
                reference,
                root_required: false,
            });
            if input.index.is_none() != bootstrap {
                return Err(LockError::Mixed(input.reference.as_ref().to_owned()));
            }
        }
        let mut network_steps = Vec::new();
        for name in &plan.order {
            let stage = &plan.stages[name];
            if let ImageRef::Remote(remote) = &stage.from {
                inputs
                    .get_mut(&ImageReference::from(remote))
                    .expect("selected source")
                    .root_required = true;
            }
            for (index, directive) in stage.derive.iter().enumerate() {
                if matches!(directive, Directive::Exec(exec) if exec.network) {
                    network_steps.push((name.clone(), index));
                }
            }
        }
        Ok(Self {
            plan,
            inputs: inputs.into_values().collect(),
            network_steps,
        })
    }
    pub(crate) fn execution_required(&self) -> bool {
        self.plan.commands_required() || self.plan.root_reader_required()
    }
    pub(crate) fn plan(&self) -> &Plan {
        &self.plan
    }
    pub(crate) fn inputs(&self) -> &[ImageInput] {
        &self.inputs
    }
    pub(crate) fn bootstrapped(&self) -> bool {
        self.inputs.iter().any(|input| input.index.is_none())
    }
    pub(crate) fn network_steps(&self) -> &[(StageRef, usize)] {
        &self.network_steps
    }
}

#[cfg(test)]
mod tests {
    use crate::{document::YamlDocument, graph::BuildGraph, lock::ImageLock, planner::Plan};
    fn graph(plan: &str) -> Result<BuildGraph, crate::lock::LockError> {
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
    fn bootstrap_tools_are_selected_without_pins_and_reused_once() {
        let selected = graph("version: 1\nimage: final\nstages:\n  first: {use: !bootstrap localhost/fedora}\n  second: {use: !bootstrap localhost/fedora}\n  final: {extend: first, steps: [{copy: {from: second, path: /x, into: /x}}]}\n  orphan: {use: !bootstrap localhost/unused}").unwrap();
        assert!(selected.bootstrapped());
        assert_eq!(selected.inputs().len(), 1);
        assert!(selected.inputs()[0].index().is_none());
        assert!(selected.inputs()[0].root_required());
        let conflict = graph(
            "version: 1\nimage: {root: fresh, boot: registry.test/base:1}\nstages: {fresh: {use: !bootstrap registry.test/base:1}}",
        );
        assert!(matches!(conflict, Err(crate::lock::LockError::Mixed(_))));
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
}
