// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{AsString, Digest};
use crate::{is_default::IsDefault, plan::Version};
use devmap_core::BlockSize;
use devmap_verity::Constraint;
use serde::{Deserialize, Serialize};

/// Chain-wide verity identity and reconstruction parameters.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CarapaceVerity {
    /// Root commitment, required exactly when the manifest contains scutes.
    #[serde(
        rename = "dev.pichi.carapace.verity.hash",
        skip_serializing_if = "Option::is_none"
    )]
    pub root: Option<Digest>,
    /// Data block size in bytes; defaults to 4096.
    #[serde(
        rename = "dev.pichi.carapace.verity.data-block-size",
        default,
        skip_serializing_if = "IsDefault::is_default"
    )]
    pub data_block_size: AsString<BlockSize<Constraint>>,
    /// Hash block size in bytes; defaults to 4096.
    #[serde(
        rename = "dev.pichi.carapace.verity.hash-block-size",
        default,
        skip_serializing_if = "IsDefault::is_default"
    )]
    pub hash_block_size: AsString<BlockSize<Constraint>>,
    /// Verity superblock version, currently 1.
    #[serde(
        rename = "dev.pichi.carapace.verity.version",
        default,
        skip_serializing_if = "IsDefault::is_default"
    )]
    pub version: AsString<Version<1>>,
}
