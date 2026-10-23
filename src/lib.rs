// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Standalone image builds from committed recipes and verified local artifacts.
//! Owns input capture, isolated execution, layer sealing, firmware/DTB processing
//! and OCI/inventory publication. Runtime qualification is tracked separately.
#![forbid(unsafe_code)]

pub mod anchor;
pub mod cache;
pub mod chain;
pub mod context;
pub mod copy;
pub mod document;
pub mod domain;
pub mod execution;
pub mod filesystem;
pub mod firmware;
pub mod formatter;
pub mod image;
pub mod input;
pub mod inventory;
pub mod isolation;
pub mod lock;
pub mod network;
pub mod output;
pub mod path;
pub mod pipeline;
pub mod plan;
pub mod policy;
pub mod process;
pub mod recipe;
pub mod request;
pub mod step;
pub mod storage;
pub mod template;
pub mod transform;
pub mod value;
