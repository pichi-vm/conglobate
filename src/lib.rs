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

#[cfg(test)]
mod surface_tests {
    /// The library must stay types-only: engine modules remain binary-private.
    #[test]
    fn public_modules_are_artifact_and_recipe_io_only() {
        let source = include_str!("lib.rs");
        let surface = source
            .split("#[cfg(test)]")
            .next()
            .expect("lib.rs surface before tests");
        for forbidden in [
            "pub mod pipeline",
            "pub mod storage",
            "pub mod isolation",
            "pub mod appliance",
            "pub mod context",
            "pub mod formatter",
            "pub mod request",
            "pub mod process",
            "pub mod network",
            "pub mod execution",
            "pub mod step",
            "pub mod inventory",
            "pub mod cache",
        ] {
            assert!(
                !surface.contains(forbidden),
                "lib must not export engine module ({forbidden})"
            );
        }
        assert!(
            surface.contains("pub mod artifact;"),
            "lib must export artifact"
        );
        assert!(
            surface.contains("pub mod recipe_io;"),
            "lib must export recipe_io"
        );
    }

    #[test]
    fn recipe_lock_and_artifact_types_resolve() {
        use crate::{
            BuildOutput, CarapaceRecipe, LockEntry, PmiRecipe, RefsLock, ScuteOut,
            artifact::{Digest, MEDIA_TYPE_PICHI_ARTIFACT_V1},
        };
        let _ = MEDIA_TYPE_PICHI_ARTIFACT_V1;
        let _ = std::mem::size_of::<Digest>();
        let _ = std::mem::size_of::<CarapaceRecipe>();
        let _ = std::mem::size_of::<PmiRecipe>();
        let _ = std::mem::size_of::<RefsLock>();
        let _ = std::mem::size_of::<LockEntry>();
        let _ = std::mem::size_of::<BuildOutput>();
        let _ = std::mem::size_of::<ScuteOut>();
    }
}
