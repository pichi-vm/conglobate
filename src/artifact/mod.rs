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

mod config;
mod digest;
mod index;
mod manifest;
mod media_type;
mod reference;

pub use config::{
    Band, CONFIG_SCHEMA_VERSION, Config, ConfigError, Ingress, Interface, Memory, PortSpec,
    Requirements,
};
pub use digest::{Digest, DigestParseError};
pub use manifest::{
    CARAPACE_ROOT_ANNOTATION, ConfigDescriptor, DtbDescriptor, Layer, Manifest,
    ManifestValidationError, PmiDescriptor, ScuteAnnotations, ScuteDescriptor,
};
pub use media_type::{
    MEDIA_TYPE_OCI_EMPTY_V1, MEDIA_TYPE_PICHI_ARTIFACT_V1, MEDIA_TYPE_PICHI_CONFIG_V1,
    MEDIA_TYPE_PICHI_DTB_V1, MEDIA_TYPE_PICHI_PMI_V1, MEDIA_TYPE_PICHI_REQUIREMENTS_V1,
    MEDIA_TYPE_PICHI_SCUTE_V1, MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
};
pub use reference::{Reference, ReferenceKind, ReferenceParseError};

pub use index::{
    Index, IndexDescriptor, LayoutMarker, MEDIA_TYPE_OCI_INDEX_V1, MEDIA_TYPE_OCI_MANIFEST_V1,
    Platform,
};
