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

mod as_string;
mod boot_config_descriptor;
mod carapace_verity;
mod compressable;
mod config;
mod config_descriptor;
mod digest;
mod index;
mod manifest;
mod manifest_annotations;
mod media;
mod media_type;
mod oci_annotations;
mod pichi_artifact;
mod reference;
mod sha1;
mod zstd;

pub use as_string::AsString;
pub use boot_config_descriptor::BootConfigDescriptor;
pub use carapace_verity::CarapaceVerity;
pub use compressable::Compressable;
pub use config::{
    Band, Config, Ingress, Interface, Memory, PortSpec, Recommendation, Requirements, Slot, Storage,
};
pub use config_descriptor::ConfigDescriptor;
pub use digest::{Digest, DigestParseError};
pub use manifest::{
    CARAPACE_ROOT_ANNOTATION, Layer, Manifest, ManifestValidationError, PmiDescriptor,
    ScuteAnnotations, ScuteDescriptor,
};
pub use manifest_annotations::ManifestAnnotations;
pub use media::Media;
pub use media_type::{MEDIA_TYPE_PICHI_REQUIREMENTS_V1, MediaType};
pub use oci_annotations::OciAnnotations;
pub use pichi_artifact::PichiArtifact;
pub use reference::{Reference, ReferenceKind, ReferenceParseError};
pub use sha1::Sha1;
pub use zstd::Zstd;

pub use index::{Index, IndexDescriptor, LayoutMarker, Platform};
