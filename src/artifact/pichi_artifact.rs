// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::Media;

/// The Pichi VM artifact format.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PichiArtifact;

impl Media for PichiArtifact {
    const MEDIA_TYPE: &'static str = "application/vnd.pichi.artifact.v1+json";
}
