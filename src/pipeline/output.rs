// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Completed stage data. No live command, mounted volume or runtime authority.
use super::{BuildError, CompletedImage};
use crate::{
    firmware::ExtractedFirmware,
    formatter::FormatReceipt,
    input::{CheckedImage, ResolvedBuild},
    inventory::Inventory,
    output::AssembledImage,
    request::BuildRequest,
    storage::SealedScute,
};
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct StageOutput<'b, 'w> {
    pub(super) base: Option<&'b CheckedImage>,
    pub(super) produced: Vec<Arc<SealedScute<'w>>>,
    pub(super) firmware: Option<ExtractedFirmware<'w>>,
    pub(super) formatter: Option<FormatReceipt>,
}
impl<'b, 'w> StageOutput<'b, 'w> {
    pub(super) fn inherited(build: &'b ResolvedBuild) -> Result<Self, BuildError> {
        let recipe = build.captured().plan().recipe();
        if recipe.bootstrap().is_some() || recipe.firmware().is_some() {
            return Err(BuildError::InvalidExecution(
                "pass-through requires no build stages",
            ));
        }
        let stage = recipe
            .carapace()
            .filter(|stage| stage.directives().is_empty())
            .ok_or(BuildError::InvalidExecution(
                "pass-through requires an empty carapace stage",
            ))?;
        let base = stage
            .from()
            .and_then(|name| build.images().get(name))
            .ok_or(BuildError::InvalidExecution("missing resolved output base"))?;
        Ok(Self {
            base: Some(base),
            produced: Vec::new(),
            firmware: None,
            formatter: None,
        })
    }

    pub(super) fn assemble(
        self,
        request: &BuildRequest,
        build: &ResolvedBuild,
    ) -> Result<CompletedImage, BuildError> {
        let Self {
            base,
            produced,
            firmware,
            formatter,
        } = self;
        let root = produced
            .last()
            .map(|scute| *scute.root())
            .or_else(|| {
                base.and_then(|image| image.chain())
                    .map(|chain| *chain.root())
            })
            .map(hex::encode);
        let transform = build.captured().plan().recipe().transform();
        let pair = firmware
            .map(|pair| pair.finalize(transform, root.as_deref()))
            .transpose()?;
        let inherited_dtb = if pair.is_none() {
            base.map(|base| base.inherited_dtb(build.cache()))
                .transpose()?
                .flatten()
        } else {
            None
        };
        let transformed;
        let dtb = if let Some(pair) = &pair {
            pair.dtb()
        } else {
            transformed = transform.apply(
                inherited_dtb.as_ref().map(|source| source.bytes()),
                root.as_deref(),
            )?;
            &transformed
        };
        let image = if let Some(pair) = &pair {
            AssembledImage::firmware(
                build.cache(),
                base.map(|base| base.image()),
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
        let inventory = Inventory::from_build(request, build, &image, dtb, &produced)?;
        let inventory = match formatter {
            Some(receipt) => inventory.with_formatter(receipt)?,
            None => inventory,
        };
        inventory.check_output(&image)?;
        Ok(CompletedImage { image, inventory })
    }
}
