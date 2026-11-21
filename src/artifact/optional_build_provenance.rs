// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::BuildProvenance;
use super::build_provenance_fields::BuildProvenanceFields;
use serde::Deserialize;

/// Decode the optional group without flatten's error-swallowing Option path.
#[derive(Deserialize)]
#[serde(remote = "Option<BuildProvenance>", try_from = "BuildProvenanceFields")]
pub(super) struct OptionalBuildProvenance;
