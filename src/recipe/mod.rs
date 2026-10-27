// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! One recipe document, validated before execution and independent of key order.
use crate::{document::YamlDocument, image::IdentityError, value::ValueError};
use serde::{Deserialize, Deserializer};
pub(crate) mod transform;
pub(crate) use transform::DEFAULT_TRANSFORM;

pub(crate) use transform::{
    PropertyValue, TransformAction, TransformOperation, TransformSelection,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum RecipeError {
    #[error(transparent)]
    Pattern(#[from] crate::copy::PatternError),
    #[error(transparent)]
    Mode(#[from] crate::copy::ModeError),
    #[error(transparent)]
    Template(#[from] crate::template::TemplateError),
    #[error("invalid recipe YAML: {0}")]
    Document(#[from] serde_saphyr::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Value(#[from] ValueError),
    #[error(transparent)]
    Schema(#[from] crate::contract::recipe::Error),
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

pub(crate) use crate::contract::recipe::{
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
    type Error = RecipeError;
    fn try_from(text: String) -> Result<Self, Self::Error> {
        if !text.starts_with('/') || text.contains('\0') {
            return Err(RecipeError::Invalid(
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
impl TryFrom<crate::contract::recipe::Directive> for Directive {
    type Error = RecipeError;
    fn try_from(raw: crate::contract::recipe::Directive) -> Result<Self, Self::Error> {
        Ok(match raw {
            crate::contract::recipe::Directive::Run(run) => Self::Run(run),
            crate::contract::recipe::Directive::Copy { copy } => Self::Copy(Copy {
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
    outputs: BTreeMap<ArtifactName, crate::contract::recipe::ImagePath>,
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
    pub(crate) fn outputs(&self) -> &BTreeMap<ArtifactName, crate::contract::recipe::ImagePath> {
        &self.outputs
    }
    pub(crate) fn firmware(&self) -> Option<&Firmware> {
        self.firmware.as_ref()
    }
}
#[derive(Debug)]
pub(crate) struct Recipe {
    tasks: BTreeMap<Local, Task>,
    image: Image,
    order: Vec<Local>,
}
impl Recipe {
    pub(crate) fn tasks(&self) -> &BTreeMap<Local, Task> {
        &self.tasks
    }
    pub(crate) fn image(&self) -> &Image {
        &self.image
    }
    pub(crate) fn order(&self) -> &[Local] {
        &self.order
    }
    pub(crate) fn transform(&self) -> &TransformSelection {
        &TransformSelection::Builtin
    }
}
impl TryFrom<YamlDocument<'_>> for Recipe {
    type Error = RecipeError;
    fn try_from(document: YamlDocument<'_>) -> Result<Self, Self::Error> {
        let raw: crate::contract::recipe::Recipe = document.decode()?;
        let order = raw.order().to_vec();
        let raw = raw.into_parts();
        let tasks = raw
            .0
            .into_iter()
            .map(|(name, task)| {
                let crate::contract::recipe::TaskParts {
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
                            .collect::<Result<_, RecipeError>>()?,
                        kind,
                        outputs: outputs.unwrap_or_default(),
                        firmware: firmware.map(|(pmi, dtb)| Firmware { pmi, dtb }),
                    },
                ))
            })
            .collect::<Result<_, RecipeError>>()?;
        Ok(Self {
            tasks,
            image: raw.1,
            order,
        })
    }
}

// Historical filesystem-operation tests retain direct construction of COPY
// policies. Those policies are deliberately absent from the new recipe schema.
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
