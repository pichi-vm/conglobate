// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ConfigError, Requirements};
use serde::{Deserialize, Serialize};

/// The config-blob schema version carried in-body. Bumped when the meaning of a
/// field changes; consumers reject anything they don't understand.
pub const CONFIG_SCHEMA_VERSION: u32 = 1;

/// The pichi artifact config blob (`application/vnd.pichi.config.v1+json`).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Schema version (see [`CONFIG_SCHEMA_VERSION`]). Required on the wire so
    /// the blob is self-identifying independent of the media-type suffix.
    pub version: u32,

    /// The host-facing launch contract.
    #[serde(default)]
    pub requirements: Requirements,
}

// Canonical field order and requirements are properties of the wire format,
// independent of the caller's choice of JSON or another serde codec.
impl Serialize for Config {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct as _;
        let mut requirements = self.requirements.clone();
        requirements.canonicalize();
        let mut state = serializer.serialize_struct("Config", 2)?;
        state.serialize_field("version", &self.version)?;
        state.serialize_field("requirements", &requirements)?;
        state.end()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_SCHEMA_VERSION,
            requirements: Requirements::default(),
        }
    }
}

impl Config {
    /// Check the schema version is understood. Requirement invariants (nonzero,
    /// 2-MiB memory, band ordering) are enforced by the field types at parse, so
    /// there is nothing else to check here. Callers validate after decoding.
    ///
    /// # Errors
    /// [`ConfigError::UnsupportedVersion`] if the version is not understood.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.version != CONFIG_SCHEMA_VERSION {
            return Err(ConfigError::UnsupportedVersion {
                got: self.version,
                want: CONFIG_SCHEMA_VERSION,
            });
        }
        Ok(())
    }
}
