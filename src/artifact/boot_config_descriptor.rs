// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// Native Linux boot configuration accompanying a PMI in an OCI image.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BootConfigDescriptor {
    pub digest: String,
    pub size: u64,
}
