// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Validated authored recipes and compiled host-copy selections.
use crate::{contract::recipe, document::YamlDocument};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub(crate) enum PlanError {
    #[error("invalid format: {0}")]
    Format(#[from] std::io::Error),
    #[error("invalid plan YAML: {0}")]
    Document(#[from] serde_saphyr::Error),
    #[error(transparent)]
    Recipe(#[from] recipe::Error),
    #[error(transparent)]
    Pattern(#[from] crate::copy::PatternError),
}

#[derive(Debug)]
pub(crate) struct Copy {
    from: Option<recipe::StageRef>,
    path: recipe::ImagePath,
    sources: Vec<crate::copy::SourcePattern>,
    destination: recipe::ImagePath,
    owner: Option<String>,
    group: Option<String>,
    mode: Option<crate::copy::CopyMode>,
}
impl Copy {
    pub(crate) fn from(&self) -> Option<&recipe::StageRef> {
        self.from.as_ref()
    }
    pub(crate) fn path(&self) -> &recipe::ImagePath {
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
pub(crate) enum Directive {
    Exec(recipe::Exec),
    Copy(Copy),
}
impl TryFrom<recipe::Directive> for Directive {
    type Error = PlanError;
    fn try_from(raw: recipe::Directive) -> Result<Self, Self::Error> {
        Ok(match raw {
            recipe::Directive::Exec(exec) => Self::Exec(exec),
            recipe::Directive::Copy { copy } => Self::Copy(Copy {
                sources: if copy.from.is_none() {
                    vec![copy.path.normalized().parse()?]
                } else {
                    vec![]
                },
                from: copy.from,
                path: copy.path,
                destination: copy.into,
                owner: None,
                group: None,
                mode: None,
            }),
        })
    }
}
#[derive(Debug)]
pub(crate) struct Stage {
    pub(crate) from: recipe::ImageRef,
    pub(crate) format: Option<recipe::Format>,
    pub(crate) mount: BTreeMap<recipe::ImagePath, recipe::Mount>,
    pub(crate) derive: Vec<Directive>,
}
impl TryFrom<recipe::Stage> for Stage {
    type Error = PlanError;
    fn try_from(raw: recipe::Stage) -> Result<Self, Self::Error> {
        let (from, format, mount, derive) = match raw {
            recipe::Stage::Create(stage) => {
                (stage.using, Some(stage.format), stage.mount, stage.derive)
            }
            recipe::Stage::Extend(stage) => (stage.extending, None, stage.mount, stage.derive),
        };
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
    pub(crate) stages: BTreeMap<recipe::StageRef, Stage>,
    pub(crate) image: recipe::Image,
    pub(crate) order: Vec<recipe::StageRef>,
}
impl Plan {
    /// The selected graph's remote inputs, including direct component selectors.
    pub(crate) fn remote_sources(&self) -> impl Iterator<Item = &recipe::Remote> {
        self.order
            .iter()
            .map(|name| &self.stages[name].from)
            .chain(self.image.sources())
            .filter_map(|source| match source {
                recipe::ImageRef::Remote(remote) => Some(remote),
                recipe::ImageRef::Local(_) => None,
            })
    }
    pub(crate) fn commands_required(&self) -> bool {
        !self.order.is_empty()
    }
    pub(crate) fn root_reader_required(&self) -> bool {
        matches!(&self.image, recipe::Image::Components(components) if matches!(components.boot, Some(recipe::Boot::File(_))))
    }
}
impl TryFrom<YamlDocument<'_>> for Plan {
    type Error = PlanError;
    fn try_from(document: YamlDocument<'_>) -> Result<Self, Self::Error> {
        let raw: recipe::Plan = document.decode()?;
        let order = raw.order()?;
        for stage in raw.stages.values() {
            if let recipe::Stage::Create(stage) = stage {
                crate::formatter::Ext4Format::from(&stage.format).layout()?;
                let recipe::Format::Ext4(format) = &stage.format;
                if let Some(esp) = format.esp {
                    crate::formatter::Fat32Format::try_from(esp)?;
                } else if stage.mount.values().any(|kind| *kind == recipe::Mount::Esp) {
                    return Err(recipe::Error::from("ESP mount requires format.esp").into());
                }
            }
        }
        let stages = raw
            .stages
            .into_iter()
            .map(|(name, stage)| Ok((name, Stage::try_from(stage)?)))
            .collect::<Result<_, PlanError>>()?;
        Ok(Self {
            stages,
            image: raw.image,
            order,
        })
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
            path: recipe::ImagePath::try_from(source.to_owned()).unwrap(),
            sources: vec![source.parse().unwrap()],
            destination: recipe::ImagePath::try_from(destination.to_owned()).unwrap(),
            owner: Some(owner),
            group: Some(group),
            mode: mode.map(|s| s.parse().unwrap()),
        }
    }
}
