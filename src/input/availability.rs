// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CheckedImage, InputError};
use crate::{
    contract::plan::{Boot, Image, ImageRef, StageRef},
    image::ImageReference,
    planner::Plan,
};
use std::collections::BTreeMap;

/// Component presence only; this never activates a source filesystem.
#[derive(Debug, Clone, Copy)]
pub(super) struct Availability {
    root: bool,
    boot: bool,
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
                let image = &images[&ImageReference::from(remote)];
                let layers = &image.image().manifest().layers;
                Ok(Self {
                    root: image.root().is_some(),
                    boot: layers.iter().any(|layer| {
                        matches!(
                            layer.media_type().as_ref(),
                            crate::artifact::MEDIA_TYPE_PICHI_PMI_V1
                        )
                    }),
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
                }
            } else {
                source
            };
            stages.insert(name.clone(), output);
        }
        let source = |reference| Self::source(reference, &stages, images);
        match &plan.image {
            Image::Shorthand(reference) => {
                let selected = source(reference)?;
                if !selected.root && !selected.boot {
                    return Err(InputError::Components(
                        "image contains neither root nor PMI".into(),
                    ));
                }
            }
            Image::Components { root, boot } => {
                if let Some(root) = root
                    && !source(root)?.root
                {
                    return Err(InputError::Components(
                        "selected root source has no carapace".into(),
                    ));
                }
                if let Some(boot) = boot {
                    match boot {
                        Boot::Extract { extract, .. } => {
                            if !source(&extract.from)?.root {
                                return Err(InputError::Components(
                                    "PMI file source has no carapace".into(),
                                ));
                            }
                        }
                        boot => {
                            let reference = match boot {
                                Boot::Shorthand(source) => source,
                                Boot::Inherit { inherit }
                                | Boot::InheritDefined { inherit, .. }
                                | Boot::InheritExtended { inherit, .. } => inherit,
                                Boot::Extract { .. } => unreachable!(),
                            };
                            if !source(reference)?.boot {
                                return Err(InputError::Components(
                                    "selected boot source has no PMI".into(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
