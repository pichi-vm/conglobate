// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Config, Digest, MediaType};
use serde::{Deserialize, Serialize};

/// The identity of a separately stored Pichi launch-contract blob.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: MediaType<Config>,
    pub digest: Digest,
    pub size: u64,
}

impl ConfigDescriptor {
    /// Describe the serialized config bytes by their measured identity.
    pub fn for_config(digest: Digest, size: u64) -> Self {
        Self {
            media_type: Default::default(),
            digest,
            size,
        }
    }
}
