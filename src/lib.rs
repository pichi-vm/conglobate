// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Portable input and output contracts shared by Pichi and Conglobate.

#![forbid(unsafe_code)]

pub mod artifact;
pub mod launch;

pub mod plan;

#[cfg(feature = "git")]
mod commit;
#[cfg(feature = "git")]
mod committed;
#[cfg(feature = "git")]
// The same private source also serves the CLI COPY engine.
#[allow(dead_code)]
mod context;
#[cfg(feature = "git")]
mod path;
