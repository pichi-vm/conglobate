// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Strict recipe types and pipeline input selection, independent of execution.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::{Error, transform::Transform};

/// A recipe reference resolved exclusively through the committed lock.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "String")]
pub struct ImageReference(String);

impl TryFrom<String> for ImageReference {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        if value.is_empty() || value.chars().any(char::is_whitespace) || value.contains('\0') {
            return Err(Error::Invalid(
                "image references must be nonempty and contain no whitespace or NUL".into(),
            ));
        }
        value.parse::<pichi_artifact::Reference>().map_err(|_| {
            Error::Invalid("image reference must use OCI syntax without URL credentials".into())
        })?;
        Ok(Self(value))
    }
}

impl AsRef<str> for ImageReference {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Linux path in the execution filesystem; always absolute and without traversal.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "String")]
pub struct GuestPath(String);

impl TryFrom<String> for GuestPath {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        if !value.starts_with('/') || value.contains('\0') || value.split('/').any(|s| s == "..") {
            return Err(Error::Invalid(format!(
                "invalid absolute guest path {value:?}"
            )));
        }
        Ok(Self(value))
    }
}
impl AsRef<str> for GuestPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// One or more paths from the committed context.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CopySources {
    One(String),
    Many(Vec<String>),
}

impl CopySources {
    pub fn paths(&self) -> &[String] {
        match self {
            Self::One(s) => std::slice::from_ref(s),
            Self::Many(s) => s,
        }
    }
    fn validate(&self) -> Result<(), Error> {
        if self.paths().is_empty()
            || self
                .paths()
                .iter()
                .any(|s| s.is_empty() || s.contains('\0') || s.split('/').any(|p| p == ".."))
        {
            return Err(Error::Invalid(
                "copy sources must be nonempty context paths without traversal".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Copy {
    pub from: CopySources,
    pub into: GuestPath,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub mode: Option<String>,
}

/// A run or copy directive. Network access is meaningful only for run.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "RawDirective")]
pub enum Directive {
    Run { command: String, network: bool },
    Copy(Copy),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDirective {
    run: Option<String>,
    copy: Option<Copy>,
    network: Option<bool>,
}
impl TryFrom<RawDirective> for Directive {
    type Error = Error;
    fn try_from(raw: RawDirective) -> Result<Self, Error> {
        match (raw.run,raw.copy,raw.network) {
            (Some(command),None,network) if !command.trim().is_empty() && !command.contains('\0') => Ok(Self::Run {command,network:network.unwrap_or(false)}),
            (None,Some(copy),None) => { copy.from.validate()?; Ok(Self::Copy(copy)) },
            _=>Err(Error::Invalid("directive requires exactly one nonempty run or copy; network is allowed only on run".into())),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bootstrap {
    pub from: ImageReference,
    #[serde(default)]
    pub derive: Vec<Directive>,
    pub into: GuestPath,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Carapace {
    pub from: Option<ImageReference>,
    #[serde(default)]
    pub derive: Vec<Directive>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirmwareOutputs {
    pub pmi: GuestPath,
    pub dtb: GuestPath,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Firmware {
    pub from: Option<ImageReference>,
    #[serde(default)]
    pub derive: Vec<Directive>,
    pub into: FirmwareOutputs,
}

/// Raw committed file contents. Presence is distinct from an empty file.
#[derive(Debug, Default)]
pub struct RecipeFiles<'a> {
    pub bootstrap: Option<&'a str>,
    pub carapace: Option<&'a str>,
    pub firmware: Option<&'a str>,
    pub platform: Option<&'a str>,
    pub transform: Option<&'a str>,
}

/// Validated stage combination. All output components propagate from `base()`.
#[derive(Debug)]
pub struct Pipeline {
    pub bootstrap: Option<Bootstrap>,
    pub carapace: Option<Carapace>,
    pub firmware: Option<Firmware>,
    pub transform: Transform,
}

impl TryFrom<RecipeFiles<'_>> for Pipeline {
    type Error = Error;
    fn try_from(files: RecipeFiles<'_>) -> Result<Self, Error> {
        if files.platform.is_some() {
            return Err(Error::Invalid(
                "platform.yaml is reserved and not implemented".into(),
            ));
        }
        let bootstrap = files
            .bootstrap
            .map(crate::yaml::from_str::<Bootstrap>)
            .transpose()?;
        let carapace = files
            .carapace
            .map(|s| crate::yaml::from_str::<Option<Carapace>>(s).map(Option::unwrap_or_default))
            .transpose()?;
        let firmware = files
            .firmware
            .map(crate::yaml::from_str::<Firmware>)
            .transpose()?;
        if bootstrap.is_none() && carapace.is_none() && firmware.is_none() {
            return Err(Error::Invalid(
                "at least one image-producing recipe is required".into(),
            ));
        }
        if let Some(carapace) = &carapace {
            if bootstrap.is_some() && carapace.from.is_some() {
                return Err(Error::Invalid(
                    "carapace.yaml cannot have from when bootstrap supplies its input".into(),
                ));
            }
            if bootstrap.is_none() && carapace.from.is_none() {
                return Err(Error::Invalid(
                    "carapace.yaml requires from without bootstrap.yaml".into(),
                ));
            }
        }
        if let Some(firmware) = &firmware {
            if bootstrap.is_none() && carapace.is_none() && firmware.from.is_none() {
                return Err(Error::Invalid(
                    "firmware-only builds require a tool from reference".into(),
                ));
            }
            if firmware.into.pmi.as_ref() == firmware.into.dtb.as_ref() {
                return Err(Error::Invalid(
                    "firmware PMI and DTB outputs must be distinct files".into(),
                ));
            }
        }
        Ok(Self {
            bootstrap,
            carapace,
            firmware,
            transform: Transform::parse(files.transform)?,
        })
    }
}

impl Pipeline {
    /// Output ancestry, not an execution-tool input. Bootstrap starts a new image.
    pub fn base(&self) -> Option<&ImageReference> {
        self.carapace.as_ref().and_then(|c| c.from.as_ref())
    }

    /// A pass-through base can contain only firmware; execution inputs cannot.
    pub(crate) fn needs_carapace(&self, reference: &ImageReference) -> bool {
        self.bootstrap
            .as_ref()
            .is_some_and(|b| &b.from == reference)
            || self
                .firmware
                .as_ref()
                .is_some_and(|f| f.from.as_ref() == Some(reference))
            || (self.base() == Some(reference)
                && (self.carapace.as_ref().is_some_and(|c| !c.derive.is_empty())
                    || self.firmware.as_ref().is_some_and(|f| f.from.is_none())))
    }

    /// References that must be pinned and available in the cache.
    pub fn references(&self) -> BTreeSet<&ImageReference> {
        self.bootstrap
            .iter()
            .map(|b| &b.from)
            .chain(self.carapace.iter().filter_map(|c| c.from.as_ref()))
            .chain(self.firmware.iter().filter_map(|f| f.from.as_ref()))
            .collect()
    }
}

/// SHA-256 identity of a pinned OCI index. Child manifests and roots are not duplicated.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct IndexDigest(String);
impl TryFrom<String> for IndexDigest {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        let hex = value
            .strip_prefix("sha256:")
            .ok_or_else(|| Error::Invalid("index digest must use sha256".into()))?;
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Invalid(
                "index digest must contain 64 lowercase hex digits".into(),
            ));
        }
        Ok(Self(value))
    }
}
impl AsRef<str> for IndexDigest {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockedImage {
    pub index: IndexDigest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageLock {
    version: u32,
    images: BTreeMap<ImageReference, LockedImage>,
}

impl ImageLock {
    pub fn parse(text: &str) -> Result<Self, Error> {
        let lock: Self = crate::yaml::from_str(text)?;
        if lock.version != 1 {
            return Err(Error::Invalid(format!(
                "unsupported lock version {}",
                lock.version
            )));
        }
        Ok(lock)
    }

    pub fn resolve(&self, reference: &ImageReference) -> Result<&IndexDigest, Error> {
        self.images
            .get(reference)
            .map(|entry| &entry.index)
            .ok_or_else(|| {
                Error::Invalid(format!("unlocked image reference {}", reference.as_ref()))
            })
    }

    pub fn validate(&self, pipeline: &Pipeline) -> Result<(), Error> {
        for reference in pipeline.references() {
            self.resolve(reference)?;
        }
        Ok(())
    }
}
