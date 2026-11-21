// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Media, MediaType, Zstd};

/// A format transported raw or with Zstd compression.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged, bound = "")]
pub enum Compressable<T: Media> {
    Raw(MediaType<T>),
    Zstd(MediaType<Zstd<T>>),
}
