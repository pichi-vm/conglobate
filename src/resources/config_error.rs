// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use thiserror::Error;

/// Errors constructing or validating a [`super::Config`].
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The blob declares a schema version this build does not understand.
    #[error("unsupported config schema version {got} (this build understands {want})")]
    UnsupportedVersion {
        /// The version found in the blob.
        got: u32,
        /// The version this build supports.
        want: u32,
    },
}
