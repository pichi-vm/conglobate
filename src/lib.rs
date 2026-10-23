// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Validated inputs and owned execution primitives for the replacement builder.
//! Full pipeline assembly and publication remain under implementation.
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
