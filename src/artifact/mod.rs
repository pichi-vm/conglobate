// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Shared, application-independent types for OCI-compatible pichi artifacts.
//!
//! Defines image manifests, component descriptors, media types, digests,
//! references, and the artifact config-blob schema. This module performs no
//! filesystem, registry, cache, VM-launch, or build orchestration. Reader-based
//! decoding accepts a caller-provided stream.
//!
//! Retired here from the standalone `pichi-artifact` crate; the wire contract
//! — media-type tags, manifest shape, digest and reference syntax — is
//! unchanged, so artifacts written against that crate stay readable.

mod annotation;
mod config;
mod digest;
mod error;
mod manifest;
mod media_type;
mod reference;

pub use annotation::{
    ANNOTATION_BUILD_COMMIT, ANNOTATION_BUILD_CONTEXT, ANNOTATION_BUILD_LOCATION,
    ANNOTATION_BUILD_LOCK, ANNOTATION_BUILD_PLAN, ANNOTATION_BUILD_PREFIX,
    ANNOTATION_CARAPACE_VERITY_ALGO, ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE,
    ANNOTATION_CARAPACE_VERITY_HASH, ANNOTATION_CARAPACE_VERITY_HASH_BLOCK_SIZE,
    ANNOTATION_CARAPACE_VERITY_HASH_TYPE, ANNOTATION_CARAPACE_VERITY_PREFIX,
    ANNOTATION_CARAPACE_VERITY_VERSION, ANNOTATION_SCUTE_VERITY_DATA_BLOCKS,
    ANNOTATION_SCUTE_VERITY_SALT,
};
pub use config::{
    Band, CONFIG_SCHEMA_VERSION, Config, ConfigError, Ingress, Interface, Memory, PortSpec,
    Requirements,
};
pub use digest::{Digest, DigestParseError};
pub use error::Error;
pub use manifest::{
    ConfigDescriptor, DtbDescriptor, Layer, Manifest, ManifestValidationError, PmiDescriptor,
    ScuteAnnotations, ScuteDescriptor,
};
pub use media_type::{
    MEDIA_TYPE_OCI_EMPTY_V1, MEDIA_TYPE_OCI_IMAGE_INDEX_V1, MEDIA_TYPE_OCI_IMAGE_MANIFEST_V1,
    MEDIA_TYPE_PICHI_ARTIFACT_V1, MEDIA_TYPE_PICHI_CONFIG_V1, MEDIA_TYPE_PICHI_DTB_V1,
    MEDIA_TYPE_PICHI_PMI_V1, MEDIA_TYPE_PICHI_REQUIREMENTS_V1, MEDIA_TYPE_PICHI_SCUTE_V1,
    MEDIA_TYPE_PICHI_SCUTE_V1_ZSTD,
};
pub use reference::{Reference, ReferenceKind, ReferenceParseError};
