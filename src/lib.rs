// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]

pub mod artifact;
pub mod console;
#[path = "plan/schema.rs"]
pub mod plan;
pub mod value;

#[cfg(test)]
mod surface_tests {
    /// The library must stay types-only: engine modules remain binary-private.
    #[test]
    fn public_surface_remains_types_only() {
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
        assert!(surface.contains("pub mod console;"));
        assert!(!surface.contains("plan_io"));
        assert!(surface.contains("pub mod plan;"));
        assert!(surface.contains("pub mod value;"));
    }

    #[test]
    fn plan_lock_console_and_artifact_types_resolve() {
        use crate::{
            artifact::{Digest, MEDIA_TYPE_PICHI_ARTIFACT_V1},
            console::{BuildReply, BuildRequest},
            plan::{Lock, Plan},
        };
        let _ = MEDIA_TYPE_PICHI_ARTIFACT_V1;
        let _ = std::mem::size_of::<Digest>();
        let _ = std::mem::size_of::<Plan>();
        let _ = std::mem::size_of::<Lock>();
        let _ = std::mem::size_of::<BuildRequest>();
        let _ = std::mem::size_of::<BuildReply>();
    }
}
