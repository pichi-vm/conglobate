// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

pub(crate) use crate::value::{CommandTimeout, FilesystemLabel, FilesystemSize};
use serde::{Deserialize, Deserializer, de};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

// deserialize_string permits YAML scalar coercion in the parser. Names and
// typed quantities must remain strings, including when used as mapping keys.
fn strict_string<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    struct Text;
    impl<'de> de::Visitor<'de> for Text {
        type Value = String;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a string")
        }
        fn visit_str<E: de::Error>(self, value: &str) -> Result<String, E> {
            Ok(value.to_owned())
        }
        fn visit_string<E: de::Error>(self, value: String) -> Result<String, E> {
            Ok(value)
        }
    }
    d.deserialize_any(Text)
}
fn nonnull<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<T, D::Error> {
    Option::<T>::deserialize(d)?
        .ok_or_else(|| de::Error::custom("null is not omission or an empty collection"))
}
#[derive(Debug, thiserror::Error)]
#[error("invalid plan: {0}")]
pub(crate) struct Error(pub(crate) String);
impl Error {
    fn invalid(text: impl fmt::Display) -> Self {
        Self(text.to_string())
    }
}

/// Local names deliberately exclude OCI separators and artifact path syntax.
/// The precise limited charset is an implementation choice, not a design lock.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Local(String);
impl FromStr for Local {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty()
            || matches!(text, "." | "..")
            || !text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        {
            return Err(Error::invalid(
                "expected a bare local name (letters, digits, _, . or -)",
            ));
        }
        Ok(Self(text.into()))
    }
}
impl AsRef<str> for Local {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for Local {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Provisional flat artifact name, kept separate from task references so the
/// open DESIGN §6.3 path-key decision can change this grammar independently.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ArtifactName(String);
impl FromStr for ArtifactName {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        text.parse::<Local>()?;
        Ok(Self(text.into()))
    }
}
impl AsRef<str> for ArtifactName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for ArtifactName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// An explicitly qualified OCI reference, retaining parsed components.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Remote {
    text: String,
}
impl AsRef<str> for Remote {
    fn as_ref(&self) -> &str {
        &self.text
    }
}
impl FromStr for Remote {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            Error::invalid(
                "remote reference requires a valid explicit registry host and OCI repository",
            )
        };
        let (registry, _) = text.split_once('/').ok_or_else(invalid)?;
        let (host, port) = if let Some(ipv6) = registry.strip_prefix('[') {
            let (address, suffix) = ipv6.split_once(']').ok_or_else(invalid)?;
            address
                .parse::<std::net::Ipv6Addr>()
                .map_err(|_| invalid())?;
            (
                address,
                if suffix.is_empty() {
                    None
                } else {
                    Some(suffix.strip_prefix(':').ok_or_else(invalid)?)
                },
            )
        } else {
            match registry.rsplit_once(':') {
                Some((host, port)) => (host, Some(port)),
                None => (registry, None),
            }
        };
        let ipv6 = registry.starts_with('[');
        if (!ipv6
            && (!(host == "localhost" || host.contains('.') || port.is_some())
                || host.split('.').any(|part| {
                    part.is_empty()
                        || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                        || part.starts_with('-')
                        || part.ends_with('-')
                })))
            || port.is_some_and(|p| {
                !p.bytes().all(|b| b.is_ascii_digit())
                    || p.parse::<u16>().ok().filter(|p| *p > 0).is_none()
            })
        {
            return Err(invalid());
        }
        let parsed: crate::artifact::Reference = text.parse().map_err(Error::invalid)?;
        if parsed.registry != registry
            || parsed.repo.split('/').any(|part| {
                part.is_empty()
                    || !part.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b)
                    })
                    || !part.as_bytes()[0].is_ascii_alphanumeric()
                    || !part.as_bytes()[part.len() - 1].is_ascii_alphanumeric()
            })
        {
            return Err(invalid());
        }
        Ok(Self { text: text.into() })
    }
}

macro_rules! string_deserialize {
    ($type:ty) => {
        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                strict_string(d)?.parse().map_err(de::Error::custom)
            }
        }
    };
}
string_deserialize!(Local);
string_deserialize!(ArtifactName);
string_deserialize!(Remote);
string_deserialize!(FilesystemSize);
string_deserialize!(CommandTimeout);

impl FromStr for Reference {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        match (text.parse::<Local>(), text.parse::<Remote>()) {
            (Ok(local), Err(_)) => Ok(Self::Local(local)),
            (Err(_), Ok(remote)) => Ok(Self::Remote(remote)),
            _ => Err(Error::invalid(
                "reference must be an unambiguous local name or fully qualified OCI reference",
            )),
        }
    }
}

/// Untagged variants have disjoint grammars; neither can shadow the other.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum Reference {
    Local(Local),
    Remote(Remote),
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Filesystem {
    Ext4(Ext4Properties),
}
/// Dedicated ext4 capacity and properties, validated before execution.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Ext4Properties {
    size: FilesystemSize,
    #[serde(default)]
    label: FilesystemLabel,
}
impl<'de> Deserialize<'de> for FilesystemLabel {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        strict_string(d)?.try_into().map_err(de::Error::custom)
    }
}
impl Filesystem {
    pub(crate) fn size(&self) -> FilesystemSize {
        match self {
            Self::Ext4(p) => p.size,
        }
    }
    pub(crate) fn label(&self) -> &FilesystemLabel {
        match self {
            Self::Ext4(p) => &p.label,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "RawRun")]
pub(crate) struct Run {
    command: String,
    network: bool,
    timeout: CommandTimeout,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRun {
    run: String,
    #[serde(default)]
    network: bool,
    #[serde(default)]
    timeout: CommandTimeout,
}
impl TryFrom<RawRun> for Run {
    type Error = Error;
    fn try_from(raw: RawRun) -> Result<Self, Error> {
        if raw.run.trim().is_empty() || raw.run.contains('\0') {
            return Err(Error::invalid("run must be nonempty without NUL"));
        }
        Ok(Self {
            command: raw.run,
            network: raw.network,
            timeout: raw.timeout,
        })
    }
}
impl Run {
    pub(crate) fn command(&self) -> &str {
        &self.command
    }
    pub(crate) fn network(&self) -> bool {
        self.network
    }
    pub(crate) fn timeout(&self) -> CommandTimeout {
        self.timeout
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Copy {
    pub(crate) from: ArtifactName,
    pub(crate) into: ImagePath,
}
#[derive(Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(crate) enum Directive {
    Run(Run),
    Copy { copy: Copy },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "String")]
pub(crate) struct ImagePath(String);
impl TryFrom<String> for ImagePath {
    type Error = Error;
    fn try_from(text: String) -> Result<Self, Error> {
        if !text.starts_with('/') || text.contains('\0') || text.split('/').any(|p| p == "..") {
            return Err(Error::invalid(
                "image path must be absolute without NUL or parent traversal",
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Bootstrap {
    from: Reference,
    filesystem: Filesystem,
    #[serde(default, rename = "exec", deserialize_with = "nonnull")]
    directives: Vec<Directive>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Extend {
    from: Reference,
    #[serde(default, rename = "exec", deserialize_with = "nonnull")]
    directives: Vec<Directive>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Build {
    from: Reference,
    #[serde(default, rename = "exec", deserialize_with = "nonnull")]
    directives: Vec<Directive>,
    #[serde(deserialize_with = "nonnull")]
    into: BTreeMap<ArtifactName, ImagePath>,
}
impl Build {
    // Provisional convention matching prior firmware paths; not a locked
    // into value grammar. Both files must be explicitly declared together.
    pub(crate) fn boot(&self) -> Option<(&str, &str)> {
        Some((
            self.into.get(&"pmi".parse().ok()?)?.as_ref(),
            self.into.get(&"dtb".parse().ok()?)?.as_ref(),
        ))
    }
}
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Task {
    Bootstrap(Bootstrap),
    Extend(Extend),
    Build(Build),
}
impl Task {
    pub(crate) fn from(&self) -> &Reference {
        match self {
            Self::Bootstrap(t) => &t.from,
            Self::Extend(t) => &t.from,
            Self::Build(t) => &t.from,
        }
    }
    pub(crate) fn directives(&self) -> &[Directive] {
        match self {
            Self::Bootstrap(t) => &t.directives,
            Self::Extend(t) => &t.directives,
            Self::Build(t) => &t.directives,
        }
    }
    pub(crate) fn has_root(&self) -> bool {
        !matches!(self, Self::Build(_))
    }
}
#[derive(Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(crate) enum Image {
    Short(Local),
    Long { root: Local, boot: Local },
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "RawPlan")]
pub(crate) struct Plan {
    tasks: BTreeMap<Local, Task>,
    image: Image,
    order: Vec<Local>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlan {
    #[serde(deserialize_with = "nonnull")]
    tasks: BTreeMap<Local, Task>,
    image: Image,
}
impl Plan {
    pub(crate) fn order(&self) -> &[Local] {
        &self.order
    }
}
impl TryFrom<RawPlan> for Plan {
    type Error = Error;
    fn try_from(raw: RawPlan) -> Result<Self, Error> {
        let mut producers = BTreeMap::new();
        for (name, task) in &raw.tasks {
            if let Task::Build(build) = task {
                if build.into.is_empty() {
                    return Err(Error::invalid("build requires declared artifacts"));
                }
                let pmi: ArtifactName = "pmi".parse()?;
                let dtb: ArtifactName = "dtb".parse()?;
                if build.into.contains_key(&pmi) != build.into.contains_key(&dtb) {
                    return Err(Error::invalid("boot requires both pmi and dtb"));
                }
                if build.boot().is_some_and(|(pmi, dtb)| pmi == dtb) {
                    return Err(Error::invalid("boot paths must be distinct"));
                }
                for artifact in build.into.keys() {
                    if producers.insert(artifact.clone(), name.clone()).is_some() {
                        return Err(Error::invalid("ambiguous artifact name"));
                    }
                }
            }
        }
        let dependencies = |name: &Local| -> Result<Vec<Local>, Error> {
            let task = raw
                .tasks
                .get(name)
                .ok_or_else(|| Error::invalid(format!("missing task {name}")))?;
            let mut deps = Vec::new();
            if let Reference::Local(source) = task.from() {
                if !raw
                    .tasks
                    .get(source)
                    .ok_or_else(|| Error::invalid(format!("missing task {source}")))?
                    .has_root()
                {
                    return Err(Error::invalid("from requires a carapace export"));
                }
                deps.push(source.clone());
            }
            for directive in task.directives() {
                if let Directive::Copy { copy } = directive
                    && let Some(producer) = producers.get(&copy.from)
                {
                    deps.push(producer.clone());
                }
            }
            Ok(deps)
        };
        // Iterative DFS bounds stack usage even for deep authored graphs.
        fn walk(
            start: &Local,
            deps: &impl Fn(&Local) -> Result<Vec<Local>, Error>,
            done: &mut BTreeSet<Local>,
            order: &mut Vec<Local>,
        ) -> Result<(), Error> {
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
                if !visiting.insert(name.clone()) {
                    return Err(Error::invalid("task dependency cycle"));
                }
                stack.push((name.clone(), true));
                for dependency in deps(&name)?.into_iter().rev() {
                    stack.push((dependency, false));
                }
            }
            Ok(())
        }
        let mut checked = BTreeSet::new();
        for name in raw.tasks.keys() {
            walk(name, &dependencies, &mut checked, &mut Vec::new())?;
        }
        let seeds = match &raw.image {
            Image::Short(name) => vec![name.clone()],
            Image::Long { root, boot } => {
                if !raw
                    .tasks
                    .get(root)
                    .ok_or_else(|| Error::invalid("missing image root task"))?
                    .has_root()
                {
                    return Err(Error::invalid("image root requires carapace export"));
                }
                vec![root.clone(), boot.clone()]
            }
        };
        // Static boot availability follows extend ancestry. Remote exports are
        // checked from verified manifests before privileged setup.
        let boot = |start: &Local| -> Result<Option<bool>, Error> {
            let mut name = start;
            loop {
                match raw
                    .tasks
                    .get(name)
                    .ok_or_else(|| Error::invalid(format!("missing task {name}")))?
                {
                    Task::Bootstrap(_) => return Ok(Some(false)),
                    Task::Build(build) => return Ok(Some(build.boot().is_some())),
                    Task::Extend(stage) => match &stage.from {
                        Reference::Local(local) => name = local,
                        Reference::Remote(_) => return Ok(None),
                    },
                }
            }
        };
        match &raw.image {
            Image::Short(name) => {
                let task = raw
                    .tasks
                    .get(name)
                    .ok_or_else(|| Error::invalid("missing image task"))?;
                if !task.has_root() && boot(name)? == Some(false) {
                    return Err(Error::invalid(
                        "image task exports neither carapace nor boot",
                    ));
                }
            }
            Image::Long { boot: name, .. } if boot(name)? == Some(false) => {
                return Err(Error::invalid("image boot requires boot export"));
            }
            _ => (),
        }
        let mut order = Vec::new();
        let mut done = BTreeSet::new();
        for seed in seeds {
            walk(&seed, &dependencies, &mut done, &mut order)?;
        }
        Ok(Self {
            tasks: raw.tasks,
            image: raw.image,
            order,
        })
    }
}

/// Owned task data for consumers that keep execution authority outside the lib.
#[derive(Debug)]
pub(crate) struct TaskParts {
    pub(crate) from: Reference,
    pub(crate) directives: Vec<Directive>,
    pub(crate) filesystem: Option<Filesystem>,
    pub(crate) outputs: Option<BTreeMap<ArtifactName, ImagePath>>,
    pub(crate) boot: Option<(String, String)>,
}
impl Task {
    pub(crate) fn into_parts(self) -> TaskParts {
        match self {
            Self::Bootstrap(t) => TaskParts {
                from: t.from,
                directives: t.directives,
                filesystem: Some(t.filesystem),
                outputs: None,
                boot: None,
            },
            Self::Extend(t) => TaskParts {
                from: t.from,
                directives: t.directives,
                filesystem: None,
                outputs: None,
                boot: None,
            },
            Self::Build(t) => {
                let boot = t.boot().map(|(pmi, dtb)| (pmi.to_owned(), dtb.to_owned()));
                TaskParts {
                    from: t.from,
                    directives: t.directives,
                    filesystem: None,
                    outputs: Some(t.into),
                    boot,
                }
            }
        }
    }
}
impl Plan {
    pub(crate) fn into_parts(self) -> (BTreeMap<Local, Task>, Image) {
        (self.tasks, self.image)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::YamlDocument;
    #[test]
    fn shipped_task_plans_parse() {
        for text in [
            include_str!("../../image/conglobate.plan.yaml"),
            include_str!("../../tests/distro/alpine/plan.yaml"),
            include_str!("../../tests/distro/debian/plan.yaml"),
            include_str!("../../tests/distro/fedora/plan.yaml"),
            include_str!("../../tests/distro/nixos/plan.yaml"),
            include_str!("../../tests/distro/pipeline/plan.yaml"),
            include_str!("../../tests/distro/pipeline/imported-boot.yaml"),
            include_str!("../../tests/distro/pipeline/boot-only.yaml"),
            include_str!("../../tests/distro/confinement/plan.yaml"),
            include_str!("../../tests/distro/network/plan.yaml"),
        ] {
            YamlDocument::from(text).decode::<Plan>().unwrap();
        }
    }
    #[test]
    fn non_string_names_and_null_collections_are_rejected() {
        for text in [
            "image: '1'\ntasks: {1: {kind: extend, from: registry.test/base:1}}",
            "image: root\ntasks: {root: {kind: extend, from: registry.test/base:1, exec: null}}",
            "image: root\ntasks: {root: {kind: extend, from: registry.test/base:1, exec: [{run: 123}]}}",
            "image: root\ntasks: {root: {kind: extend, from: registry.test/base:1, exec: [{copy: {from: 123, into: /x}}]}}",
        ] {
            assert!(YamlDocument::from(text).decode::<Plan>().is_err(), "{text}");
        }
    }
    #[test]
    fn strict_reference_keeps_no_implicit_hub_defaults() {
        let reference: Reference = "registry.test/base:1".parse().unwrap();
        let Reference::Remote(remote) = reference else {
            panic!()
        };
        assert_eq!(remote.as_ref(), "registry.test/base:1");
    }
}
