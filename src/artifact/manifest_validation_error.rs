// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::resources::MEDIA_TYPE_PICHI_CONFIG_V1;
use thiserror::Error;

/// Validation errors for [`super::ManifestExt::validate`] (per D-07).
#[derive(Debug, Error)]
pub enum ManifestValidationError {
    /// The artifact contains a component Pichi does not support.
    #[error("unsupported artifact layer media type: {0}")]
    UnsupportedLayer(String),

    /// D-07 rule 1: wrong `artifactType`.
    #[error("artifactType must be {expected}, got {actual}")]
    WrongArtifactType {
        /// Expected value.
        expected: String,
        /// Actual value found.
        actual: String,
    },
    /// D-07 rule 2: config descriptor is neither the canonical OCI empty config
    /// nor a `vnd.pichi.config.v1+json` blob.
    #[error(
        "config descriptor mediaType {media_type} is not the OCI empty config or {MEDIA_TYPE_PICHI_CONFIG_V1}"
    )]
    BadConfig {
        /// The offending config mediaType.
        media_type: String,
    },
    /// D-07 rule 3: required chain annotation key is absent.
    #[error("missing required chain annotation: {0}")]
    MissingChainAnnotation(&'static str),
    /// A chain annotation is present but malformed (e.g. non-integer
    /// version/hash-type). This is a well-formedness check only — pichi imposes
    /// no value whitelist (that is the guest consumer's control per the spec).
    #[error("chain annotation {key} is malformed: {value}")]
    BadChainAnnotation {
        /// The annotation key.
        key: &'static str,
        /// The offending value.
        value: String,
    },
    /// D-07 rule 5: more than one PMI layer.
    #[error("artifact has more than one PMI layer (got {0}); per D-03 at most one is permitted")]
    MultiplePmi(usize),

    /// More than one boot configuration layer.
    #[error("artifact has more than one boot configuration layer (got {0})")]
    MultipleBootConfig(usize),

    /// Boot configuration cannot be distributed independently of firmware.
    #[error("boot configuration requires a PMI")]
    BootConfigWithoutPmi,

    /// Scute salt is not valid hex or lacks the 32-byte parent root prefix.
    #[error("scute salt must contain a hex-encoded parent root: {0}")]
    BadSalt(String),

    /// The first scute must be the base; later scutes must have a parent.
    #[error("scutes must appear in base-to-tip order: {0}")]
    BadScuteOrder(String),

    /// Declared data-block count is zero or contradicts a raw blob's size.
    #[error("invalid data-block count for scute {0}")]
    BadScuteBlocks(String),
}
