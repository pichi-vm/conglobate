// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Portable plan and lock encoding. Documents require `version: 1`.
//! Execution checks belong to the consumer.
//!
//! String wrappers validate through `new(String) -> Option<Self>` and Serde.
//! They provide display, borrowed string access, equality, ordering and hashing;
//! their private storage cannot be mutated into an invalid value.

mod boot;
mod boot_config;
mod directive;
mod extract;
mod filesystem;
mod filesystem_size;
mod format;
mod image;
mod image_path;
mod image_ref;
mod mount;
mod remote;
mod stage;
mod stage_ref;
mod version;

pub use boot::Boot;
pub use boot_config::{Key, Value};
pub use directive::Directive;
pub use extract::Extract;
pub use filesystem::Filesystem;
pub use filesystem_size::FilesystemSize;
pub use format::Format;
pub use image::Image;
pub use image_path::ImagePath;
pub use image_ref::ImageRef;
pub use mount::Mount;
pub use remote::Remote;
pub use stage::Stage;
pub use stage_ref::StageRef;
pub use version::Version;

use crate::artifact::Digest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Authored encoding. Execution converts this into a resolved, checked plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub version: Version<1>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stages: BTreeMap<StageRef, Stage>,
    pub image: Image,
}

/// Remote image references pinned to immutable OCI indexes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Lock {
    pub version: Version<1>,
    /// Each digest identifies a multi-architecture OCI index.
    pub images: BTreeMap<Remote, Digest>,
}
