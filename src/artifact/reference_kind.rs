// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::Digest;

/// Either a human-readable tag or a content digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReferenceKind {
    /// Tag string (e.g. `latest`, `3`, `v1.2.3`).
    Tag(String),
    /// Content digest.
    Digest(Digest),
}
