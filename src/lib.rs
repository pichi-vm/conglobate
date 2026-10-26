// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Shared, engine-free types for the conglobate build contract.
//!
//! This library is the surface downstream consumers depend on. It carries
//! data types and their parse/serialise rules only: no sandboxing, no mounts,
//! no process isolation, no Git object-store or registry I/O. Decoding bytes
//! that a caller already holds is in scope; acquiring them is not.
//!
//! The build engine itself — pipeline, storage, isolation, step execution,
//! process supervision, networking, context capture, formatters, the CLI
//! request and the appliance entry point — stays private to the
//! `conglobate` binary and is deliberately absent here.
#![forbid(unsafe_code)]

pub mod artifact;
pub mod recipe_io;

pub use recipe_io::{
    BuildOutput, CarapaceRecipe, CopyDirective, Directive, FromSpec, LockEntry, PmiRecipe,
    RecipeError, RefsLock, ScuteOut,
};
