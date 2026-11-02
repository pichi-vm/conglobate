// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CompletedImage, boot_config::BootConfig, pmi::Pmi, root::Root};
use crate::{
    artifact::Layer,
    contract::plan::{Boot, Image, ImageRef, StageRef},
    image::ImageReference,
    input::{CheckedImage, ResolvedBuild},
    output::AssembledImage,
    request::BuildRequest,
    storage::{Runtime, Workspace},
};
use anyhow::{anyhow, ensure};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone)]
pub(crate) struct StageOutput<'b, 'w> {
    pub(crate) root: Option<Root<'b, 'w>>,
    pub(crate) pmi: Option<Pmi<'b, 'w>>,
    pub(crate) bootconfig: Option<BootConfig>,
}
impl<'b, 'w> StageOutput<'b, 'w> {
    pub(super) fn remote(base: &'b CheckedImage, build: &ResolvedBuild) -> anyhow::Result<Self> {
        let has_boot = base
            .image()
            .manifest()
            .layers
            .iter()
            .any(|layer| matches!(layer, Layer::Pmi(_)));
        let config = base
            .bootconfig(build.cache())?
            .map(|bytes| BootConfig::inherited(bytes.bytes().to_vec(), base.root()))
            .transpose()?
            .filter(|config| !config.bytes.is_empty());
        Ok(Self {
            root: base.root().map(|_| Root {
                base: Some(base),
                produced: vec![],
            }),
            pmi: has_boot.then_some(Pmi::Inherited(base)),
            bootconfig: config,
        })
    }
    pub(super) fn resolve(
        reference: &ImageRef,
        build: &'b ResolvedBuild,
        outputs: &BTreeMap<StageRef, Self>,
    ) -> anyhow::Result<Self> {
        match reference {
            ImageRef::Local(name) => outputs
                .get(name)
                .cloned()
                .ok_or_else(|| anyhow!("stage {name} has not completed")),
            ImageRef::Remote(remote) => Self::remote(
                build
                    .images()
                    .get(&ImageReference::from(remote))
                    .ok_or_else(|| anyhow!("remote source is not resolved"))?,
                build,
            ),
        }
    }
    pub(super) fn bind(&mut self) -> anyhow::Result<()> {
        if let Some(root) = &self.root
            && let Some(config) = &mut self.bootconfig
        {
            config.bind(root.hash())?;
        }
        Ok(())
    }
    pub(super) fn select(
        build: &'b ResolvedBuild,
        outputs: &BTreeMap<StageRef, Self>,
        reader: Option<(&'w Workspace, &Runtime<'_>)>,
    ) -> anyhow::Result<Self> {
        let plan = build.captured().graph().plan();
        let resolve = |reference| Self::resolve(reference, build, outputs);
        let mut output = match &plan.image {
            Image::Shorthand(reference) => resolve(reference)?,
            Image::Components { root, boot } => {
                let root = root
                    .as_ref()
                    .map(|reference| {
                        resolve(reference)?
                            .root
                            .ok_or_else(|| anyhow!("selected source has no root"))
                    })
                    .transpose()?;
                let (pmi, bootconfig) = match boot {
                    None => (None, None),
                    Some(Boot::Extract { extract, define }) => {
                        let source = resolve(&extract.from)?
                            .root
                            .ok_or_else(|| anyhow!("PMI file source has no root"))?;
                        let (workspace, runtime) = reader
                            .ok_or_else(|| anyhow!("PMI extraction requires a reader runtime"))?;
                        let pmi = Pmi::File(Arc::new(source.read(
                            build.cache(),
                            workspace,
                            runtime,
                            |volume, budget| {
                                Ok(volume.extract_artifact(
                                    extract.path.as_ref(),
                                    workspace,
                                    budget,
                                )?)
                            },
                        )?));
                        let config = BootConfig::overlay(
                            None,
                            define.iter().map(|(key, value)| (key, Some(value))),
                        )?;
                        (Some(pmi), Some(config))
                    }
                    Some(boot) => {
                        let reference = match boot {
                            Boot::Shorthand(source) => source,
                            Boot::Inherit { inherit }
                            | Boot::InheritDefined { inherit, .. }
                            | Boot::InheritExtended { inherit, .. } => inherit,
                            Boot::Extract { .. } => unreachable!(),
                        };
                        let source = resolve(reference)?;
                        let pmi = source
                            .pmi
                            .ok_or_else(|| anyhow!("selected source has no PMI"))?;
                        let config = match boot {
                            Boot::InheritDefined { define, .. } => Some(BootConfig::overlay(
                                None,
                                define.iter().map(|(key, value)| (key, Some(value))),
                            )?),
                            Boot::InheritExtended { extend, .. } => Some(BootConfig::overlay(
                                source.bootconfig,
                                extend.iter().map(|(key, value)| (key, value.as_ref())),
                            )?),
                            _ => source.bootconfig,
                        };
                        (Some(pmi), config)
                    }
                };
                Self {
                    root,
                    pmi,
                    bootconfig: bootconfig.filter(|config| !config.bytes.is_empty()),
                }
            }
        };
        ensure!(
            output.root.is_some() || output.pmi.is_some(),
            "image contains neither root nor PMI"
        );
        ensure!(
            output.bootconfig.is_none() || output.pmi.is_some(),
            "bootconfig requires PMI"
        );
        if let Some(root) = &output.root {
            let hash = Some(root.hash());
            let changed = !root.produced.is_empty()
                || output.pmi.as_ref().is_some_and(|pmi| pmi.root() != hash)
                || output
                    .bootconfig
                    .as_ref()
                    .is_some_and(|config| config.root != hash);
            if changed {
                output.bind()?;
            }
        }
        Ok(output)
    }
    pub(super) fn assemble(
        self,
        request: &BuildRequest,
        build: &ResolvedBuild,
    ) -> anyhow::Result<CompletedImage> {
        let image = AssembledImage::components(build.cache(), &self, request.output())?
            .bind_build(request, build.captured())?;
        Ok(CompletedImage { image })
    }
}

#[cfg(test)]
#[path = "../../tests/unit/composition.rs"]
mod tests;
