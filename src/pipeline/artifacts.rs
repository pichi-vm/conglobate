// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Finalized regular-file artifacts, private to this build's RAM workspace.
use crate::{firmware::FirmwareFile, recipe::ArtifactName};
use std::collections::BTreeMap;
pub(crate) type Artifacts<'w> = BTreeMap<ArtifactName, FirmwareFile<'w>>;
