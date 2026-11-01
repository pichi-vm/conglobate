// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CheckedImage, InputError};
use crate::{
    contract::recipe::{Boot, BootConfigSource, Image, ImageRef, StageRef},
    image::ImageReference,
    plan::Plan,
};
use std::collections::BTreeMap;

/// Component presence only; this never activates a source filesystem.
#[derive(Debug, Clone, Copy)]
pub(super) struct Availability {
    root: bool,
    boot: bool,
    config: bool,
}
impl Availability {
    fn source(
        reference: &ImageRef,
        stages: &BTreeMap<StageRef, Self>,
        images: &BTreeMap<ImageReference, CheckedImage>,
    ) -> Result<Self, InputError> {
        match reference {
            ImageRef::Local(name) => stages
                .get(name)
                .copied()
                .ok_or_else(|| InputError::Components(format!("stage {name} is not available"))),
            ImageRef::Remote(remote) => {
                let image = &images[&remote.as_ref().parse::<ImageReference>()?];
                let layers = &image.image().manifest().layers;
                Ok(Self {
                    root: image.root().is_some(),
                    boot: layers
                        .iter()
                        .any(|layer| matches!(layer, crate::artifact::Layer::Pmi(_))),
                    config: layers
                        .iter()
                        .any(|layer| matches!(layer, crate::artifact::Layer::BootConfig(_))),
                })
            }
        }
    }
    pub(super) fn validate(
        plan: &Plan,
        images: &BTreeMap<ImageReference, CheckedImage>,
    ) -> Result<(), InputError> {
        let mut stages = BTreeMap::new();
        for name in &plan.order {
            let stage = &plan.stages[name];
            let source = Self::source(&stage.from, &stages, images)?;
            if !source.root {
                return Err(InputError::Components(format!(
                    "stage {name} requires a carapace input"
                )));
            }
            let output = if stage.format.is_some() {
                Self {
                    root: true,
                    boot: false,
                    config: false,
                }
            } else {
                source
            };
            stages.insert(name.clone(), output);
        }
        let source = |reference| Self::source(reference, &stages, images);
        match &plan.image {
            Image::Inherit(reference) => {
                let selected = source(reference)?;
                if !selected.root && !selected.boot {
                    return Err(InputError::Components(
                        "image contains neither root nor PMI".into(),
                    ));
                }
            }
            Image::Components(components) => {
                if let Some(root) = &components.root
                    && !source(root)?.root
                {
                    return Err(InputError::Components(
                        "selected root source has no carapace".into(),
                    ));
                }
                if let Some(boot) = &components.boot {
                    match boot {
                        Boot::Inherit(reference) if !source(reference)?.boot => {
                            return Err(InputError::Components(
                                "selected boot source has no PMI".into(),
                            ));
                        }
                        Boot::File(file) if !source(&file.from)?.root => {
                            return Err(InputError::Components(
                                "PMI file source has no carapace".into(),
                            ));
                        }
                        _ => (),
                    }
                }
                let from = components
                    .bootconfig
                    .as_ref()
                    .and_then(|config| match config {
                        BootConfigSource::Inherit(reference) => Some(reference),
                        BootConfigSource::Overlay(overlay) => overlay.from.as_ref(),
                    });
                if let Some(from) = from
                    && !source(from)?.config
                {
                    return Err(InputError::Components(
                        "selected bootconfig source has no bootconfig".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}
