// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Validated authored plans and compiled host-copy selections.
use crate::{contract::plan, document::YamlDocument};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum PlanError {
    #[error("invalid format: {0}")]
    Format(#[from] std::io::Error),
    #[error("invalid plan YAML: {0}")]
    Document(#[from] serde_saphyr::Error),
    #[error("invalid plan: {0}")]
    Invalid(String),
    #[error(transparent)]
    Pattern(#[from] crate::copy::PatternError),
}

#[derive(Debug)]
pub(crate) struct Copy {
    from: Option<plan::StageRef>,
    path: plan::ImagePath,
    sources: Vec<crate::copy::SourcePattern>,
    destination: plan::ImagePath,
    owner: Option<String>,
    group: Option<String>,
    mode: Option<crate::copy::CopyMode>,
}
impl Copy {
    pub(crate) fn from(&self) -> Option<&plan::StageRef> {
        self.from.as_ref()
    }
    pub(crate) fn path(&self) -> &plan::ImagePath {
        &self.path
    }
    pub(crate) fn sources(&self) -> &[crate::copy::SourcePattern] {
        &self.sources
    }
    pub(crate) fn destination(&self) -> &str {
        self.destination.as_ref()
    }
    pub(crate) fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }
    pub(crate) fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }
    pub(crate) fn mode(&self) -> Option<&crate::copy::CopyMode> {
        self.mode.as_ref()
    }
}
#[derive(Debug)]
pub(crate) struct Exec {
    pub(crate) exec: String,
    pub(crate) network: bool,
    pub(crate) timeout: Duration,
}
#[derive(Debug)]
pub(crate) enum Directive {
    Exec(Exec),
    Copy(Copy),
}
impl TryFrom<plan::Directive> for Directive {
    type Error = PlanError;
    fn try_from(raw: plan::Directive) -> Result<Self, Self::Error> {
        Ok(match raw {
            plan::Directive::Exec {
                command,
                network,
                timeout,
            } => Self::Exec(Exec {
                exec: command,
                network,
                timeout,
            }),
            plan::Directive::Copy { from, path, into } => Self::Copy(Copy {
                sources: if from.is_none() {
                    vec![path.normalized().parse()?]
                } else {
                    vec![]
                },
                from,
                path,
                destination: into,
                owner: None,
                group: None,
                mode: None,
            }),
        })
    }
}
#[derive(Debug)]
pub(crate) struct Stage {
    pub(crate) from: plan::ImageRef,
    pub(crate) format: Option<plan::Format>,
    pub(crate) mount: BTreeMap<plan::ImagePath, plan::Mount>,
    pub(crate) derive: Vec<Directive>,
}
impl TryFrom<plan::Stage> for Stage {
    type Error = PlanError;
    fn try_from(raw: plan::Stage) -> Result<Self, Self::Error> {
        let (from, format, mount, derive) = match raw {
            plan::Stage::Create {
                r#use,
                format,
                mounts,
                steps,
            } => (r#use, Some(format), mounts, steps),
            plan::Stage::Extend {
                extend,
                mounts,
                steps,
            } => (extend, None, mounts, steps),
        };
        let mut paths = BTreeSet::new();
        for path in mount.keys() {
            let normalized = path.normalized();
            let first = normalized
                .trim_start_matches('/')
                .split('/')
                .next()
                .unwrap_or("");
            if normalized == "/"
                || (format.is_none() && matches!(first, "dev" | "proc" | "sys"))
                || !paths.insert(normalized)
            {
                return Err(PlanError::Invalid(
                    "mounts cannot replace the root, runtime controls, or another mount".into(),
                ));
            }
        }
        if let Some(spec) = &format {
            crate::formatter::Ext4Format::from(spec).layout()?;
            if let Some(esp) = spec.esp {
                crate::formatter::Fat32Format::try_from(esp)?;
            } else if mount.values().any(|kind| *kind == plan::Mount::Esp) {
                return Err(PlanError::Invalid("ESP mount requires format.esp".into()));
            }
        }
        Ok(Self {
            from,
            format,
            mount,
            derive: derive
                .into_iter()
                .map(Directive::try_from)
                .collect::<Result<_, _>>()?,
        })
    }
}
#[derive(Debug)]
pub(crate) struct Plan {
    pub(crate) stages: BTreeMap<plan::StageRef, Stage>,
    pub(crate) image: plan::Image,
    pub(crate) order: Vec<plan::StageRef>,
}
impl Plan {
    /// The selected graph's remote inputs, including direct component selectors.
    pub(crate) fn remote_sources(&self) -> impl Iterator<Item = &plan::Remote> {
        self.order
            .iter()
            .map(|name| &self.stages[name].from)
            .chain(self.image_sources())
            .filter_map(|source| match source {
                plan::ImageRef::Remote(remote) => Some(remote),
                plan::ImageRef::Local(_) => None,
            })
    }
    pub(crate) fn commands_required(&self) -> bool {
        !self.order.is_empty()
    }
    pub(crate) fn root_reader_required(&self) -> bool {
        matches!(
            &self.image,
            plan::Image::Components {
                boot: Some(plan::Boot::Extract { .. }),
                ..
            }
        )
    }
    fn image_sources(&self) -> Vec<&plan::ImageRef> {
        match &self.image {
            plan::Image::Shorthand(source) => vec![source],
            plan::Image::Components { root, boot } => root
                .iter()
                .chain(boot.iter().map(|boot| match boot {
                    plan::Boot::Shorthand(source) => source,
                    plan::Boot::Inherit { inherit }
                    | plan::Boot::InheritDefined { inherit, .. }
                    | plan::Boot::InheritExtended { inherit, .. } => inherit,
                    plan::Boot::Extract { extract, .. } => &extract.from,
                }))
                .collect(),
        }
    }
    fn walk(
        &self,
        start: &plan::StageRef,
        done: &mut BTreeSet<plan::StageRef>,
        order: &mut Vec<plan::StageRef>,
    ) -> Result<(), PlanError> {
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
                .ok_or_else(|| PlanError::Invalid(format!("unknown stage {name}")))?;
            if !visiting.insert(name.clone()) {
                return Err(PlanError::Invalid(format!(
                    "stage dependency cycle at {name}"
                )));
            }
            let mut dependencies = BTreeSet::new();
            if let plan::ImageRef::Local(name) = &stage.from {
                dependencies.insert(name.clone());
            }
            for directive in &stage.derive {
                if let Directive::Copy(copy) = directive
                    && let Some(name) = copy.from()
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
impl TryFrom<plan::Plan> for Plan {
    type Error = PlanError;
    fn try_from(raw: plan::Plan) -> Result<Self, Self::Error> {
        if matches!(
            raw.image,
            plan::Image::Components {
                root: None,
                boot: None
            }
        ) {
            return Err(PlanError::Invalid(
                "image contains neither root nor PMI".into(),
            ));
        }
        let stages = raw
            .stages
            .into_iter()
            .map(|(name, stage)| Ok((name, Stage::try_from(stage)?)))
            .collect::<Result<_, PlanError>>()?;
        let mut plan = Self {
            stages,
            image: raw.image,
            order: Vec::new(),
        };
        let mut checked = BTreeSet::new();
        for name in plan.stages.keys() {
            plan.walk(name, &mut checked, &mut Vec::new())?;
        }
        let mut order = Vec::new();
        let mut done = BTreeSet::new();
        for source in plan.image_sources() {
            if let plan::ImageRef::Local(name) = source {
                plan.walk(name, &mut done, &mut order)?;
            }
        }
        plan.order = order;
        Ok(plan)
    }
}
impl TryFrom<YamlDocument<'_>> for Plan {
    type Error = PlanError;
    fn try_from(document: YamlDocument<'_>) -> Result<Self, Self::Error> {
        Self::try_from(document.decode::<plan::Plan>()?)
    }
}

// Filesystem tests exercise metadata policies independently of the YAML schema.
#[cfg(test)]
impl Copy {
    pub(crate) fn fixture(
        source: &str,
        destination: &str,
        owner: String,
        group: String,
        mode: Option<String>,
    ) -> Self {
        Self {
            from: None,
            path: plan::ImagePath::new(source.to_owned()).unwrap(),
            sources: vec![source.parse().unwrap()],
            destination: plan::ImagePath::new(destination.to_owned()).unwrap(),
            owner: Some(owner),
            group: Some(group),
            mode: mode.map(|s| s.parse().unwrap()),
        }
    }
}

#[cfg(test)]
mod tests {
    // SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
    // SPDX-License-Identifier: Apache-2.0

    use crate::{
        document::YamlDocument,
        planner::{Directive, Plan},
    };
    fn parse(text: &str) -> Result<Plan, crate::planner::PlanError> {
        Plan::try_from(YamlDocument::from(text))
    }
    #[test]
    fn creation_exec_and_component_selection_are_accepted() {
        let plan = parse("version: 1\nimage: rootfs\nstages: {rootfs: {use: registry.test/tools:1, steps: [{exec: {command: install, network: true, timeout: 5m}}]}}").unwrap();
        assert_eq!(plan.order[0].as_ref(), "rootfs");
        let Directive::Exec(exec) = &plan.stages.values().next().unwrap().derive[0] else {
            panic!()
        };
        assert!(exec.network);
        assert_eq!(exec.timeout.as_secs(), 300);
        assert!(parse("version: 1\nimage: {root: rootfs, boot: {extract: {from: firmware, path: /boot.pmi}}}\nstages: {rootfs: {extend: registry.test/root:1}, firmware: {extend: rootfs}}").is_ok());
    }
    #[test]
    fn old_vocabulary_and_ambiguous_shapes_are_rejected() {
        for text in [
            "{}",
            "version: 1\nimage: {}",
            "version: 1\nimage: missing",
            "bootstrap: {}",
            "carapace: {}",
            "firmware: {}",
            "transform: []",
            "platform: {}",
            "lock: {}",
            "version: 1\nimage: registry.test/base:1\ntasks: {}",
            "version: 1\nimage: null",
            "version: 1\nimage: registry.test/base:1\nstages: null",
            "version: 1\nimage: x\nimage: x\nstages: {}",
            "version: 1\nimage: x\nstages: {x: {extend: a, extend: b}}",
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
                parse(&format!(
                    "version: 1\nimage: x\nstages: {{x: {{{fields}}}}}"
                ))
                .is_err(),
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
                "version: 1\nimage: x\nstages: {{x: {{extend: registry.test/base:1, steps: {steps}}}}}"
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
            assert!(parse(&format!("version: 1\nimage: registry.test/base:1\nstages: {{unused: {{extend: registry.test/base:1, mounts: {mounts}}}}}")).is_err());
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
                "version: 1\nimage: x\nstages: {{x: {{use: registry.test/tools:1, format: {{{format}}}}}}}"
            ))
            .is_err(),
            "{format}"
        );
        }
        assert!(
        parse(
            "version: 1\nimage: x\nstages: {x: {use: registry.test/tools:1, mounts: {/efi: esp}}}"
        )
        .is_err()
    );
        assert!(parse("version: 1\nimage: x\nstages: {x: {use: registry.test/tools:1, format: {esp: 512MiB}, mounts: {/efi: esp, /tmp: tmpfs}}}").is_ok());
    }
    #[test]
    fn graph_orders_copies_and_rejects_unknown_sources_and_unused_cycles() {
        let plan = parse("version: 1\nimage: result\nstages:\n  orphan: {extend: missing.registry/test:1}\n  result: {extend: base, steps: [{copy: {from: producer, path: /out, into: /payload}}]}\n  producer: {extend: base}\n  base: {extend: registry.test/base:1}").unwrap();
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
            assert!(
                parse(&format!(
                    "version: 1\nimage: registry.test/base:1\nstages: {stages}"
                ))
                .is_err()
            );
        }
    }
}
