// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Resolve the complete recipe input set before privileged execution begins.

use crate::{
    Error,
    cache::{Cache, ResolvedImage},
    context::Context,
    recipe::{ImageReference, Pipeline},
};
use std::{collections::BTreeMap, path::Path};

/// Validated source and image descriptions. Execution must still verify scute
/// chain roots and copy blobs into private storage before consuming them.
#[derive(Debug)]
pub struct Inputs {
    pub context: Context,
    pub pipeline: Pipeline,
    pub images: BTreeMap<ImageReference, ResolvedImage>,
}

impl Inputs {
    pub fn resolve(
        repository: &Path,
        commit: &str,
        subdirectory: &Path,
        cache: &Cache,
    ) -> Result<Self, Error> {
        let context = Context::capture(repository, commit, subdirectory)?;
        let (pipeline, lock) = context.pipeline()?;
        let mut images = BTreeMap::new();
        for reference in pipeline.references() {
            let image = cache.resolve(lock.resolve(reference)?)?;
            if pipeline.needs_carapace(reference) && image.manifest.carapace_verity_hash().is_none()
            {
                return Err(Error::Invalid(format!(
                    "execution image {} has no carapace",
                    reference.as_ref()
                )));
            }
            cache.verify_blobs(&image.manifest)?;
            images.insert(reference.clone(), image);
        }
        Ok(Self {
            context,
            pipeline,
            images,
        })
    }
}
