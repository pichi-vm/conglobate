// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! One recipe document, validated before execution and independent of key order.
use crate::{
    document::YamlDocument,
    image::{IdentityError, ImageReference},
    value::{CommandTimeout, FilesystemLabel, FilesystemSize, ValueError},
};
use serde::{Deserialize, Deserializer};
mod transform;
use transform::TransformDocument;
pub use transform::{PropertyValue, TransformAction, TransformOperation, TransformSelection};

#[derive(Debug, thiserror::Error)]
pub enum RecipeError {
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Document {
    #[serde(default)]
    bootstrap: Presence<BootstrapDocument>,
    #[serde(default)]
    carapace: Presence<CarapaceDocument>,
    #[serde(default)]
    firmware: Presence<FirmwareDocument>,
    #[serde(default)]
    transform: Presence<Vec<TransformDocument>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BootstrapDocument {
    from: String,
    filesystem: FilesystemDocument,
    #[serde(default)]
    derive: Presence<Vec<DirectiveDocument>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FilesystemDocument {
    #[serde(rename = "type")]
    kind: String,
    size: String,
    #[serde(default)]
    properties: Presence<PropertiesDocument>,
}
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PropertiesDocument {
    #[serde(default)]
    label: Presence<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CarapaceDocument {
    #[serde(default)]
    from: Presence<String>,
    #[serde(default)]
    derive: Presence<Vec<DirectiveDocument>>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FirmwareDocument {
    #[serde(default)]
    from: Presence<String>,
    #[serde(default)]
    derive: Presence<Vec<DirectiveDocument>>,
    into: OutputsDocument,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OutputsDocument {
    pmi: String,
    dtb: String,
}
#[derive(Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum DirectiveDocument {
    Run {
        run: String,
        #[serde(default)]
        network: Presence<bool>,
        #[serde(default)]
        timeout: Presence<String>,
    },
    Copy {
        copy: CopyDocument,
    },
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CopyDocument {
    from: SourcesDocument,
    into: String,
    #[serde(default)]
    owner: Presence<String>,
    #[serde(default)]
    group: Presence<String>,
    #[serde(default)]
    mode: Presence<String>,
}
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum SourcesDocument {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug)]
pub struct FilesystemSpec {
    size: FilesystemSize,
    label: FilesystemLabel,
}
impl TryFrom<FilesystemDocument> for FilesystemSpec {
    type Error = RecipeError;
    fn try_from(raw: FilesystemDocument) -> Result<Self, Self::Error> {
        if raw.kind != "ext4" {
            return Err(RecipeError::Invalid("unsupported filesystem type"));
        }
        Ok(Self {
            size: raw.size.parse()?,
            label: raw
                .properties
                .into_option()
                .unwrap_or_default()
                .label
                .into_option()
                .unwrap_or_default()
                .try_into()?,
        })
    }
}
impl FilesystemSpec {
    pub fn size(&self) -> FilesystemSize {
        self.size
    }
    pub fn label(&self) -> &FilesystemLabel {
        &self.label
    }
}

#[derive(Debug)]
pub struct Run {
    command: String,
    network: bool,
    timeout: CommandTimeout,
}
impl Run {
    pub fn command(&self) -> &str {
        &self.command
    }
    pub fn network(&self) -> bool {
        self.network
    }
    pub fn timeout(&self) -> CommandTimeout {
        self.timeout
    }
}
#[derive(Debug)]
pub struct Copy {
    sources: Vec<crate::copy::SourcePattern>,
    destination: ImagePath,
    owner: Option<String>,
    group: Option<String>,
    mode: Option<crate::copy::CopyMode>,
}
impl Copy {
    pub fn sources(&self) -> &[crate::copy::SourcePattern] {
        &self.sources
    }
    pub fn destination(&self) -> &str {
        self.destination.as_ref()
    }
    pub fn owner(&self) -> Option<&str> {
        self.owner.as_deref()
    }
    pub fn group(&self) -> Option<&str> {
        self.group.as_deref()
    }
    pub fn mode(&self) -> Option<&crate::copy::CopyMode> {
        self.mode.as_ref()
    }
}
#[derive(Debug)]
pub enum Directive {
    Run(Run),
    Copy(Copy),
}
impl TryFrom<DirectiveDocument> for Directive {
    type Error = RecipeError;
    fn try_from(raw: DirectiveDocument) -> Result<Self, Self::Error> {
        match raw {
            DirectiveDocument::Run {
                run,
                network,
                timeout,
            } => {
                if run.trim().is_empty() || run.contains('\0') {
                    return Err(RecipeError::Invalid(
                        "run must be nonempty and contain no NUL",
                    ));
                }
                Ok(Self::Run(Run {
                    command: run,
                    network: network.into_option().unwrap_or(false),
                    timeout: timeout
                        .into_option()
                        .map(|v| v.parse())
                        .transpose()?
                        .unwrap_or_default(),
                }))
            }
            DirectiveDocument::Copy { copy } => {
                let sources = match copy.from {
                    SourcesDocument::One(path) => vec![path],
                    SourcesDocument::Many(paths) => paths,
                };
                if sources.is_empty() {
                    return Err(RecipeError::Invalid("COPY requires at least one source"));
                }
                let sources = sources
                    .into_iter()
                    .map(|source| source.parse())
                    .collect::<Result<Vec<_>, crate::copy::PatternError>>()?;
                let destination = ImagePath::try_from(copy.into)?;
                if sources.len() > 1 && !destination.as_ref().ends_with('/') {
                    return Err(RecipeError::Invalid(
                        "multiple COPY sources require a directory destination ending in /",
                    ));
                }
                let owner = copy.owner.into_option();
                let group = copy.group.into_option();
                let mode = copy.mode.into_option().map(|s| s.parse()).transpose()?;
                if [owner.as_deref(), group.as_deref()]
                    .into_iter()
                    .flatten()
                    .any(|s| s.is_empty() || s.contains('\0'))
                {
                    return Err(RecipeError::Invalid(
                        "copy attributes must be nonempty and contain no NUL",
                    ));
                }
                Ok(Self::Copy(Copy {
                    sources,
                    destination,
                    owner,
                    group,
                    mode,
                }))
            }
        }
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
pub struct Bootstrap {
    from: ImageReference,
    filesystem: FilesystemSpec,
    directives: Vec<Directive>,
}
#[derive(Debug)]
pub struct Carapace {
    from: Option<ImageReference>,
    directives: Vec<Directive>,
}
#[derive(Debug)]
pub struct Firmware {
    from: Option<ImageReference>,
    directives: Vec<Directive>,
    pmi: ImagePath,
    dtb: ImagePath,
}
impl Bootstrap {
    pub fn from(&self) -> &ImageReference {
        &self.from
    }
    pub fn filesystem(&self) -> &FilesystemSpec {
        &self.filesystem
    }
    pub fn directives(&self) -> &[Directive] {
        &self.directives
    }
}
impl Carapace {
    pub fn from(&self) -> Option<&ImageReference> {
        self.from.as_ref()
    }
    pub fn directives(&self) -> &[Directive] {
        &self.directives
    }
}
impl Firmware {
    pub fn from(&self) -> Option<&ImageReference> {
        self.from.as_ref()
    }
    pub fn directives(&self) -> &[Directive] {
        &self.directives
    }
    pub fn pmi(&self) -> &str {
        self.pmi.as_ref()
    }
    pub fn dtb(&self) -> &str {
        self.dtb.as_ref()
    }
}
#[derive(Debug)]
pub struct Recipe {
    bootstrap: Option<Bootstrap>,
    carapace: Option<Carapace>,
    firmware: Option<Firmware>,
    transform: TransformSelection,
}
impl Recipe {
    pub fn bootstrap(&self) -> Option<&Bootstrap> {
        self.bootstrap.as_ref()
    }
    pub fn carapace(&self) -> Option<&Carapace> {
        self.carapace.as_ref()
    }
    pub fn firmware(&self) -> Option<&Firmware> {
        self.firmware.as_ref()
    }
    pub fn transform(&self) -> &TransformSelection {
        &self.transform
    }
}
impl TryFrom<YamlDocument<'_>> for Recipe {
    type Error = RecipeError;
    fn try_from(document: YamlDocument<'_>) -> Result<Self, Self::Error> {
        let raw: Document = document.decode()?;
        let bootstrap = raw
            .bootstrap
            .into_option()
            .map(|s| {
                Ok::<_, RecipeError>(Bootstrap {
                    from: s.from.parse()?,
                    filesystem: s.filesystem.try_into()?,
                    directives: s
                        .derive
                        .into_option()
                        .unwrap_or_default()
                        .into_iter()
                        .map(Directive::try_from)
                        .collect::<Result<_, _>>()?,
                })
            })
            .transpose()?;
        let carapace = raw
            .carapace
            .into_option()
            .map(|s| {
                Ok::<_, RecipeError>(Carapace {
                    from: s.from.into_option().map(|r| r.parse()).transpose()?,
                    directives: s
                        .derive
                        .into_option()
                        .unwrap_or_default()
                        .into_iter()
                        .map(Directive::try_from)
                        .collect::<Result<_, _>>()?,
                })
            })
            .transpose()?;
        let firmware = raw
            .firmware
            .into_option()
            .map(|s| {
                if s.into.pmi == s.into.dtb {
                    return Err(RecipeError::Invalid(
                        "firmware output paths must be distinct",
                    ));
                }
                Ok::<_, RecipeError>(Firmware {
                    from: s.from.into_option().map(|r| r.parse()).transpose()?,
                    directives: s
                        .derive
                        .into_option()
                        .unwrap_or_default()
                        .into_iter()
                        .map(Directive::try_from)
                        .collect::<Result<_, _>>()?,
                    pmi: ImagePath::try_from(s.into.pmi)?,
                    dtb: ImagePath::try_from(s.into.dtb)?,
                })
            })
            .transpose()?;
        if bootstrap.is_none() && carapace.is_none() && firmware.is_none() {
            return Err(RecipeError::Invalid("recipe has no image-producing stage"));
        }
        if let Some(stage) = &carapace {
            if bootstrap.is_some() && stage.from.is_some() {
                return Err(RecipeError::Invalid(
                    "carapace.from is forbidden after bootstrap",
                ));
            }
            if bootstrap.is_none() && stage.from.is_none() {
                return Err(RecipeError::Invalid(
                    "carapace.from is required without bootstrap",
                ));
            }
        }
        if let Some(stage) = &firmware
            && bootstrap.is_none()
            && carapace.is_none()
            && stage.from.is_none()
        {
            return Err(RecipeError::Invalid(
                "firmware.from is required without preceding output",
            ));
        }
        let transform = match raw.transform {
            Presence::Absent => TransformSelection::Builtin,
            Presence::Present(operations) => TransformSelection::Authored(
                operations
                    .into_iter()
                    .map(TransformOperation::try_from)
                    .collect::<Result<_, _>>()?,
            ),
        };
        Ok(Self {
            bootstrap,
            carapace,
            firmware,
            transform,
        })
    }
}
