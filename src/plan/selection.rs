// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Authored dependency validation and selected-stage order.

use super::{Boot, Directive, Image, ImageRef, Plan, Stage, StageRef, Tool};
use std::collections::BTreeSet;

impl Plan {
    /// Whether the selected image closure contains an enabled network command.
    ///
    /// Validates all declared stage dependencies, including unused stages.
    /// This does not validate execution resources or resolve image locks.
    pub fn requires_network(&self) -> Result<bool, String> {
        Ok(self.stage_order()?.iter().any(|name| {
            let steps = match &self.stages[name] {
                Stage::Create { steps, .. } | Stage::Extend { steps, .. } => steps,
            };
            steps
                .iter()
                .any(|step| matches!(step, Directive::Exec { network: true, .. }))
        }))
    }

    /// Validate dependencies and return the selected stages in execution order.
    ///
    /// Unknown stages, dependency cycles and an empty image selection are errors.
    pub fn stage_order(&self) -> Result<Vec<StageRef>, String> {
        let sources: Vec<_> = match &self.image {
            Image::Shorthand(source) => vec![source],
            Image::Components { root, boot } => root
                .iter()
                .chain(boot.iter().map(|boot| match boot {
                    Boot::Shorthand(source) => source,
                    Boot::Inherit { inherit }
                    | Boot::InheritDefined { inherit, .. }
                    | Boot::InheritExtended { inherit, .. } => inherit,
                    Boot::Extract { extract, .. } => &extract.from,
                }))
                .collect(),
        };
        if sources.is_empty() {
            return Err("image contains neither root nor PMI".into());
        }
        let mut checked = BTreeSet::new();
        for name in self.stages.keys() {
            self.walk(name, &mut checked, &mut Vec::new())?;
        }
        let mut order = Vec::new();
        let mut done = BTreeSet::new();
        for source in sources {
            if let ImageRef::Local(name) = source {
                self.walk(name, &mut done, &mut order)?;
            }
        }
        Ok(order)
    }

    fn walk(
        &self,
        start: &StageRef,
        done: &mut BTreeSet<StageRef>,
        order: &mut Vec<StageRef>,
    ) -> Result<(), String> {
        let mut visiting = BTreeSet::new();
        let mut stack = vec![(start.clone(), false)];
        while let Some((name, exit)) = stack.pop() {
            if exit {
                visiting.remove(&name);
                done.insert(name.clone());
                order.push(name);
                continue;
            }
            if done.contains(&name) {
                continue;
            }
            let stage = self
                .stages
                .get(&name)
                .ok_or_else(|| format!("unknown stage {name}"))?;
            if !visiting.insert(name.clone()) {
                return Err(format!("stage dependency cycle at {name}"));
            }
            let (source, steps) = match stage {
                Stage::Create { r#use, steps, .. } => (
                    match r#use {
                        Tool::Image(source) => Some(source),
                        Tool::Bootstrap(_) => None,
                    },
                    steps,
                ),
                Stage::Extend { extend, steps, .. } => (Some(extend), steps),
            };
            let mut dependencies = BTreeSet::new();
            if let Some(ImageRef::Local(name)) = source {
                dependencies.insert(name.clone());
            }
            for directive in steps {
                if let Directive::Copy {
                    from: Some(name), ..
                } = directive
                {
                    dependencies.insert(name.clone());
                }
            }
            stack.push((name, true));
            stack.extend(dependencies.into_iter().rev().map(|name| (name, false)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn plan(stages: Value, image: Value) -> Plan {
        serde_json::from_value(json!({"version":1,"stages":stages,"image":image})).unwrap()
    }

    #[test]
    fn only_selected_enabled_commands_require_network() {
        for network in [None, Some(false), Some(true)] {
            let mut exec = json!({"command":"true"});
            if let Some(value) = network {
                exec["network"] = json!(value);
            }
            let stages = json!({
                "selected":{"extend":"registry.test/base:1","steps":[{"exec":exec}]},
                "unused":{"extend":"registry.test/base:1","steps":[{"exec":{"command":"true","network":true}}]}
            });
            assert_eq!(
                plan(stages.clone(), json!("selected"))
                    .requires_network()
                    .unwrap(),
                network == Some(true)
            );
            assert!(
                !plan(stages, json!("registry.test/base:1"))
                    .requires_network()
                    .unwrap()
            );
        }
        assert!(
            !plan(json!({}), json!("registry.test/base:1"))
                .requires_network()
                .unwrap()
        );
    }

    #[test]
    fn dependencies_and_both_component_selectors_share_execution_order() {
        let stages = json!({
            "online":{"extend":"registry.test/base:1","steps":[{"exec":{"command":"true","network":true}}]},
            "created":{"use":"online"},
            "extended":{"extend":"online"},
            "copied":{"extend":"registry.test/base:1","steps":[{"copy":{"from":"created","path":"/out","into":"/out"}}]},
            "result":{"extend":"extended","steps":[{"copy":{"from":"copied","path":"/out","into":"/out"}}]}
        });
        let selected = plan(stages.clone(), json!("result"));
        assert_eq!(
            selected
                .stage_order()
                .unwrap()
                .iter()
                .map(|s| s.as_ref())
                .collect::<Vec<_>>(),
            ["online", "created", "copied", "extended", "result"]
        );
        assert!(selected.requires_network().unwrap());
        for boot in [
            json!("online"),
            json!({"inherit":"online"}),
            json!({"inherit":"online","define":{}}),
            json!({"inherit":"online","extend":{}}),
            json!({"extract":{"from":"online","path":"/boot.pmi"}}),
        ] {
            assert!(
                plan(
                    stages.clone(),
                    json!({"root":"registry.test/base:1","boot":boot})
                )
                .requires_network()
                .unwrap()
            );
        }
        assert!(
            plan(
                stages,
                json!({"root":"online","boot":"registry.test/base:1"})
            )
            .requires_network()
            .unwrap()
        );
    }

    #[test]
    fn malformed_dependencies_fail_even_when_unused() {
        for stages in [
            json!({"x":{"extend":"missing"}}),
            json!({"x":{"extend":"y"},"y":{"extend":"x"}}),
            json!({"x":{"use":"x"}}),
            json!({"x":{"extend":"registry.test/base:1","steps":[{"copy":{"from":"missing","path":"/out","into":"/out"}}]}}),
        ] {
            for image in [json!("x"), json!("registry.test/base:1")] {
                assert!(plan(stages.clone(), image).requires_network().is_err());
            }
        }
        assert!(
            plan(json!({}), json!("missing"))
                .requires_network()
                .is_err()
        );
        assert!(plan(json!({}), json!({})).requires_network().is_err());
    }
}
