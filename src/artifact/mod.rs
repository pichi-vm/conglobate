// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Pichi artifact media types, annotations and validation.
//!
//! OCI representations come from `oci_spec`. Resource configuration lives in
//! [`crate::resources`]. This module defines only Pichi's component rules.

mod manifest_ext;
mod manifest_validation_error;
mod media_type;
mod scute_annotations;

pub use manifest_ext::{CARAPACE_ROOT_ANNOTATION, ManifestExt};
pub use manifest_validation_error::ManifestValidationError;
pub use media_type::{
    MEDIA_TYPE_PICHI_ARTIFACT_V1, MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD, MEDIA_TYPE_PICHI_PMI_V1,
    MEDIA_TYPE_PICHI_SCUTE_V1, MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
};
pub use scute_annotations::ScuteAnnotations;
