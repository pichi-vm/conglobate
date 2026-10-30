// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::CompletedImage;
use crate::{
    input::{CheckedImage, ResolvedBuild},
    output::AssembledImage,
    plan::{Image, Local, Plan, Reference},
    request::BuildRequest,
    storage::{ReadOnlyChain, SealedScute, Workspace},
};
use anyhow::{anyhow, ensure};
use std::{collections::BTreeMap, sync::Arc};
#[derive(Debug)]
pub(super) struct StageOutput<'b, 'w> {
    pub(super) base: Option<&'b CheckedImage>,
    pub(super) produced: Vec<Arc<SealedScute<'w>>>,
    pub(super) firmware: Option<crate::firmware::FinalizedFirmware<'w>>,
    pub(super) boot_base: Option<&'b CheckedImage>,
}
impl Clone for StageOutput<'_, '_> {
    fn clone(&self) -> Self {
        Self {
            base: self.base,
            produced: self.produced.clone(),
            firmware: self.firmware.clone(),
            boot_base: self.boot_base,
        }
    }
}
impl<'b, 'w> StageOutput<'b, 'w> {
    pub(super) fn remote(base: &'b CheckedImage) -> Self {
        let has_boot = base
            .image()
            .manifest()
            .layers
            .iter()
            .any(|l| matches!(l, crate::artifact::Layer::Pmi(_)));
        Self {
            base: Some(base),
            produced: vec![],
            firmware: None,
            boot_base: has_boot.then_some(base),
        }
    }
    pub(super) fn root(&self) -> Option<[u8; 32]> {
        self.produced
            .last()
            .map(|s| *s.root())
            .or_else(|| self.base.and_then(|b| b.chain()).map(|c| *c.root()))
    }
    pub(super) fn activate(
        &self,
        build: &'b ResolvedBuild,
        workspace: &'w Workspace,
    ) -> anyhow::Result<ReadOnlyChain<'b, 'w>> {
        let base = self.base.and_then(|base| base.chain());
        if base.is_none() && self.produced.is_empty() {
            return Err(anyhow!("task input requires a carapace"));
        }
        Ok(ReadOnlyChain::assemble(
            base,
            &self.produced,
            build.cache(),
            workspace,
        )?)
    }
    pub(super) fn rewrite(
        &mut self,
        build: &ResolvedBuild,
        workspace: &'w Workspace,
    ) -> anyhow::Result<()> {
        if self.firmware.is_none() {
            self.firmware = self
                .boot_base
                .map(|image| {
                    crate::firmware::FinalizedFirmware::inherited(image, build.cache(), workspace)
                })
                .transpose()?
                .flatten();
            self.boot_base = None;
        }
        if let Some(pair) = &self.firmware {
            self.firmware = Some(pair.rewrite(self.root().map(hex::encode).as_deref())?);
        }
        Ok(())
    }
    pub(super) fn select(plan: &Plan, outputs: &BTreeMap<Local, Self>) -> anyhow::Result<Self> {
        let mut output = match plan.image() {
            Image::Short(name) => outputs
                .get(name)
                .ok_or(anyhow!("missing image task"))?
                .clone(),
            Image::Long { root, boot } => {
                let mut output = outputs
                    .get(root)
                    .ok_or(anyhow!("missing root task"))?
                    .clone();
                ensure!(output.root().is_some(), "image root task has no carapace");
                let boot = outputs.get(boot).ok_or(anyhow!("missing boot task"))?;
                ensure!(
                    boot.firmware.is_some() || boot.boot_base.is_some(),
                    "image boot task has no boot export"
                );
                output.firmware = boot.firmware.clone();
                output.boot_base = boot.boot_base;
                output
            }
        };
        ensure!(
            output.root().is_some() || output.firmware.is_some() || output.boot_base.is_some(),
            "image task exports neither carapace nor boot"
        );
        // A boot-only build exports no implicit tool ancestry.
        if output.root().is_none() {
            output.base = None;
        }
        Ok(output)
    }
    pub(super) fn inherited(build: &'b ResolvedBuild) -> anyhow::Result<Self> {
        let plan = build.captured().graph().plan();
        let mut outputs = BTreeMap::new();
        for name in plan.order() {
            let task = &plan.tasks()[name];
            ensure!(
                matches!(task.kind(), crate::plan::TaskKind::Extend)
                    && task.directives().is_empty(),
                "pass-through requires empty extend tasks"
            );
            let output = match task.from() {
                Reference::Local(local) => outputs
                    .get(local)
                    .cloned()
                    .ok_or(anyhow!("missing local input"))?,
                Reference::Remote(remote) => Self::remote(
                    build
                        .images()
                        .get(&remote.as_ref().parse()?)
                        .ok_or(anyhow!("missing remote input"))?,
                ),
            };
            outputs.insert(name.clone(), output);
        }
        Self::select(plan, &outputs)
    }
    pub(super) fn assemble(
        self,
        request: &BuildRequest,
        build: &ResolvedBuild,
    ) -> anyhow::Result<CompletedImage> {
        let root = self.root().map(hex::encode);
        let Self {
            base,
            produced,
            firmware,
            boot_base,
        } = self;
        // Capture separately selected inherited boot without promoting its root
        // or configuration into the output ancestry. No command runtime is used.
        let workspace = if firmware.is_none() && boot_base.is_some() && !same_base(base, boot_base)
        {
            Some(Workspace::create()?)
        } else {
            None
        };
        let imported = if let Some(workspace) = &workspace {
            crate::firmware::FinalizedFirmware::inherited(
                boot_base.expect("selected boot"),
                build.cache(),
                workspace,
            )?
        } else {
            None
        };
        let pair = firmware
            .as_ref()
            .or(imported.as_ref())
            .map(|p| p.rewrite(root.as_deref()))
            .transpose()?;
        let inherited_dtb = if pair.is_none() {
            boot_base
                .map(|b| b.inherited_dtb(build.cache()))
                .transpose()?
                .flatten()
        } else {
            None
        };
        let transformed;
        let dtb = if let Some(pair) = &pair {
            pair.dtb()
        } else {
            transformed = crate::plan::TransformSelection::Builtin
                .apply(inherited_dtb.as_ref().map(|s| s.bytes()), root.as_deref())?;
            &transformed
        };
        let image = if let Some(pair) = &pair {
            AssembledImage::firmware(
                build.cache(),
                base.map(|b| b.image()),
                pair,
                &produced,
                request.output(),
            )?
        } else if let Some(base) = base {
            AssembledImage::derived(
                build.cache(),
                base.image(),
                &produced,
                request.output(),
                dtb,
            )?
        } else {
            AssembledImage::bootstrap_layers(&produced, request.output())?
        };
        let image = image.bind_build(request, build.captured())?;
        Ok(CompletedImage { image })
    }
}
fn same_base(left: Option<&CheckedImage>, right: Option<&CheckedImage>) -> bool {
    match (left, right) {
        (Some(a), Some(b)) => std::ptr::eq(a, b),
        (None, None) => true,
        _ => false,
    }
}
