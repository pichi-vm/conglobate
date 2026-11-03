// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Shared, application-independent types for OCI-compatible pichi artifacts.
//!
//! Defines image manifests, component descriptors, media types, digests,
//! references, and the artifact config-blob schema. This module performs no
//! filesystem, registry, cache, VM-launch, hashing, or codec operations.
//! Callers serialize and deserialize these types with their chosen serde codec.
//!
//! Retired here from the standalone `pichi-artifact` crate. The v1 format uses
//! these shared types, including required scute lengths and a manifest chain tip.

mod boot_config_descriptor;
mod manifest;
mod media_type;

pub use boot_config_descriptor::BootConfigDescriptor;
pub use manifest::{
    CARAPACE_ROOT_ANNOTATION, Layer, Manifest, ManifestValidationError, PmiDescriptor,
    ScuteAnnotations, ScuteDescriptor,
};
pub use media_type::{
    MEDIA_TYPE_PICHI_ARTIFACT_V1, MEDIA_TYPE_PICHI_BOOTCONFIG_V1_ZSTD, MEDIA_TYPE_PICHI_PMI_V1,
    MEDIA_TYPE_PICHI_SCUTE_V1, MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
};
