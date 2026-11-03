// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! One plan document, validated before execution and independent of key order.
use crate::{document::YamlDocument, image::IdentityError, value::ValueError};
use serde::{Deserialize, Deserializer};
pub(crate) mod schema;
pub(crate) mod transform;

pub(crate) use transform::{
    PropertyValue, TransformAction, TransformOperation, TransformSelection,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum PlanError {
    #[error(transparent)]
    Pattern(#[from] crate::copy::PatternError),
    #[error(transparent)]
    Mode(#[from] crate::copy::ModeError),
    #[error(transparent)]
    Template(#[from] crate::template::TemplateError),
    #[error("invalid plan YAML: {0}")]
    Document(#[from] serde_saphyr::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Value(#[from] ValueError),
    #[error(transparent)]
    Schema(#[from] crate::planner::schema::Error),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("invalid regular expression: {0}")]
    Regex(#[from] regex::Error),
}

// Presence must survive parsing: null is not omission or an empty section.
#[derive(Debug, Default)]
pub(crate) enum Presence<T> {
    #[default]
    Absent,
    Present(T),
}
impl<'de, T: Deserialize<'de>> Deserialize<'de> for Presence<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_option(PresenceVisitor(std::marker::PhantomData))
    }
}
#[derive(Debug)]
struct PresenceVisitor<T>(std::marker::PhantomData<T>);
impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for PresenceVisitor<T> {
    type Value = Presence<T>;
    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an explicit non-null value")
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Err(E::custom("null is not omission or an empty section"))
    }
    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        T::deserialize(deserializer).map(Presence::Present)
    }
}
impl<T> Presence<T> {
    fn into_option(self) -> Option<T> {
        match self {
            Self::Absent => None,
            Self::Present(value) => Some(value),
        }
    }
}

pub(crate) use crate::planner::schema::{
    ArtifactName, Filesystem as FilesystemSpec, Image, Local, Reference, Run,
};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Copy {
    sources: Vec<crate::copy::SourcePattern>,
    destination: ImagePath,
    owner: Option<String>,
    group: Option<String>,
    mode: Option<crate::copy::CopyMode>,
}
impl Copy {
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
struct ImagePath(String);
impl TryFrom<String> for ImagePath {
    type Error = PlanError;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        if !text.starts_with('/') || text.contains('\0') {
            return Err(PlanError::Invalid(
                "image path must be absolute and contain no NUL",
            ));
        }
        Ok(Self(text))
    }
}
impl AsRef<str> for ImagePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) enum Directive {
    Run(Run),
    Copy(Copy),
}
impl TryFrom<crate::planner::schema::Directive> for Directive {
    type Error = PlanError;
    fn try_from(raw: crate::planner::schema::Directive) -> Result<Self, Self::Error> {
        Ok(match raw {
            crate::planner::schema::Directive::Run(run) => Self::Run(run),
            crate::planner::schema::Directive::Copy { copy } => Self::Copy(Copy {
                sources: vec![copy.from.as_ref().parse()?],
                destination: ImagePath::try_from(copy.into.as_ref().to_owned())?,
                owner: None,
                group: None,
                mode: None,
            }),
        })
    }
}
impl Copy {
    pub(crate) fn artifact(&self) -> &str {
        self.sources[0].source()
    }
}
#[derive(Debug)]
pub(crate) struct Bootstrap {
    filesystem: FilesystemSpec,
}
impl Bootstrap {
    pub(crate) fn filesystem(&self) -> &FilesystemSpec {
        &self.filesystem
    }
}
#[derive(Debug)]
pub(crate) struct Firmware {
    pmi: String,
    dtb: String,
}
impl Firmware {
    pub(crate) fn pmi(&self) -> &str {
        &self.pmi
    }
    pub(crate) fn dtb(&self) -> &str {
        &self.dtb
    }
}
#[derive(Debug)]
pub(crate) enum TaskKind {
    Bootstrap(Bootstrap),
    Extend,
    Build,
}
#[derive(Debug)]
pub(crate) struct Task {
    from: Reference,
    kind: TaskKind,
    directives: Vec<Directive>,
    outputs: BTreeMap<ArtifactName, crate::planner::schema::ImagePath>,
    firmware: Option<Firmware>,
}
impl Task {
    pub(crate) fn from(&self) -> &Reference {
        &self.from
    }
    pub(crate) fn kind(&self) -> &TaskKind {
        &self.kind
    }
    pub(crate) fn directives(&self) -> &[Directive] {
        &self.directives
    }
    pub(crate) fn outputs(&self) -> &BTreeMap<ArtifactName, crate::planner::schema::ImagePath> {
        &self.outputs
    }
    pub(crate) fn firmware(&self) -> Option<&Firmware> {
        self.firmware.as_ref()
    }
}
#[derive(Debug)]
pub(crate) struct Plan {
    tasks: BTreeMap<Local, Task>,
    image: Image,
    order: Vec<Local>,
}
impl Plan {
    pub(crate) fn tasks(&self) -> &BTreeMap<Local, Task> {
        &self.tasks
    }
    pub(crate) fn image(&self) -> &Image {
        &self.image
    }
    pub(crate) fn order(&self) -> &[Local] {
        &self.order
    }
}
impl TryFrom<YamlDocument<'_>> for Plan {
    type Error = PlanError;
    fn try_from(document: YamlDocument<'_>) -> Result<Self, Self::Error> {
        let raw: crate::planner::schema::Plan = document.decode()?;
        let order = raw.order().to_vec();
        let raw = raw.into_parts();
        let tasks = raw
            .0
            .into_iter()
            .map(|(name, task)| {
                let crate::planner::schema::TaskParts {
                    from,
                    directives,
                    filesystem,
                    outputs,
                    boot: firmware,
                } = task.into_parts();
                let kind = match filesystem {
                    Some(filesystem) => TaskKind::Bootstrap(Bootstrap { filesystem }),
                    None if outputs.is_some() => TaskKind::Build,
                    None => TaskKind::Extend,
                };
                Ok((
                    name,
                    Task {
                        from,
                        directives: directives
                            .into_iter()
                            .map(Directive::try_from)
                            .collect::<Result<_, PlanError>>()?,
                        kind,
                        outputs: outputs.unwrap_or_default(),
                        firmware: firmware.map(|(pmi, dtb)| Firmware { pmi, dtb }),
                    },
                ))
            })
            .collect::<Result<_, PlanError>>()?;
        Ok(Self {
            tasks,
            image: raw.1,
            order,
        })
    }
}

// Historical filesystem-operation tests retain direct construction of COPY
// policies. Those policies are deliberately absent from the new plan schema.
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
            sources: vec![source.parse().unwrap()],
            destination: ImagePath::try_from(destination.to_owned()).unwrap(),
            owner: Some(owner),
            group: Some(group),
            mode: mode.map(|s| s.parse().unwrap()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        document::YamlDocument,
        planner::{Directive, Plan},
    };
    fn parse(text: &str) -> Result<Plan, crate::planner::PlanError> {
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
        use crate::planner::schema::{Local, Reference, Remote};
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

    #[test]
    fn malformed_patterns_and_parent_traversal_fail_during_plan_parsing() {
        for source in ["[", "[]", "[a-]", "\\", "[^]", "../file", "dir/../file", ""] {
            let yaml = format!(
                "image: root\ntasks: {{root: {{kind: extend, from: registry.test/base:1, exec: [{{copy: {{from: '{source}', into: /file}}}}]}}}}"
            );
            assert!(
                Plan::try_from(YamlDocument::from(yaml.as_str())).is_err(),
                "{source:?}"
            );
        }
    }
}
