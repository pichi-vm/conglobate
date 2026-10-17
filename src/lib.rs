// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Build contracts and in-process transformations for the standalone builder.

#![forbid(unsafe_code)]

pub mod artifact;
pub mod build;
pub mod cache;
pub mod cancel;
pub mod context;
pub mod copy;
mod copy_pattern;
pub mod execute;
pub mod firmware;
pub mod input;
pub mod inventory;
pub mod mounts;
pub mod paths;
pub mod recipe;
pub mod scute;
pub mod snapshot;
mod template;
pub mod transform;
mod yaml;

/// A malformed build input or failed transformation.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Linux operation: {0}")]
    Linux(#[from] nix::errno::Errno),

    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid YAML: {0}")]
    Yaml(#[from] serde_saphyr::Error),

    #[error("{0}")]
    Invalid(String),

    #[error("DTB: {0}")]
    Dtb(String),
}
