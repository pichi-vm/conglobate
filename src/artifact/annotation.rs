// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Annotation names used on pichi artifact manifests.
//!
//! An annotation is a named text value stored in an OCI manifest or layer
//! descriptor. Conglobate writes these names and Pichi reads them, so both
//! sides take them from here rather than spelling them out.

/// Common prefix of every build annotation. A new build replaces all of
/// them, so readers can drop inherited ones by this prefix.
pub const ANNOTATION_BUILD_PREFIX: &str = "dev.pichi.build.";
/// The Git commit the build used, as `<algorithm>:<hex>`.
pub const ANNOTATION_BUILD_COMMIT: &str = "dev.pichi.build.commit";
/// The build context directory inside the repository (`.` for the root).
pub const ANNOTATION_BUILD_CONTEXT: &str = "dev.pichi.build.context";
/// The plan file path inside the repository.
pub const ANNOTATION_BUILD_PLAN: &str = "dev.pichi.build.plan";
/// The lock file path inside the repository.
pub const ANNOTATION_BUILD_LOCK: &str = "dev.pichi.build.lock";
/// Where the repository came from, when the request named it.
pub const ANNOTATION_BUILD_LOCATION: &str = "dev.pichi.build.location";

/// Common prefix of the manifest-wide carapace verity annotations.
pub const ANNOTATION_CARAPACE_VERITY_PREFIX: &str = "dev.pichi.carapace.verity.";
/// Hash algorithm of the verity tree (for example `sha256`).
pub const ANNOTATION_CARAPACE_VERITY_ALGO: &str = "dev.pichi.carapace.verity.algo";
/// Size in bytes of each verity data block.
pub const ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE: &str =
    "dev.pichi.carapace.verity.data-block-size";
/// Size in bytes of each verity hash block.
pub const ANNOTATION_CARAPACE_VERITY_HASH_BLOCK_SIZE: &str =
    "dev.pichi.carapace.verity.hash-block-size";
/// Verity superblock version.
pub const ANNOTATION_CARAPACE_VERITY_VERSION: &str = "dev.pichi.carapace.verity.version";
/// Verity superblock hash type.
pub const ANNOTATION_CARAPACE_VERITY_HASH_TYPE: &str = "dev.pichi.carapace.verity.hash-type";
/// Root hash of the newest layer. It is no longer written: readers work
/// out the newest layer from the layers themselves. Writers remove any
/// copy they inherit.
pub const ANNOTATION_CARAPACE_VERITY_HASH: &str = "dev.pichi.carapace.verity.hash";

/// Per-layer verity salt, as hex.
pub const ANNOTATION_SCUTE_VERITY_SALT: &str = "dev.pichi.scute.verity.salt";
/// Per-layer count of verity data blocks.
pub const ANNOTATION_SCUTE_VERITY_DATA_BLOCKS: &str = "dev.pichi.scute.verity.data-blocks";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_share_their_prefixes() {
        for name in [
            ANNOTATION_BUILD_COMMIT,
            ANNOTATION_BUILD_CONTEXT,
            ANNOTATION_BUILD_PLAN,
            ANNOTATION_BUILD_LOCK,
            ANNOTATION_BUILD_LOCATION,
        ] {
            assert!(name.starts_with(ANNOTATION_BUILD_PREFIX), "{name}");
        }
        for name in [
            ANNOTATION_CARAPACE_VERITY_ALGO,
            ANNOTATION_CARAPACE_VERITY_DATA_BLOCK_SIZE,
            ANNOTATION_CARAPACE_VERITY_HASH_BLOCK_SIZE,
            ANNOTATION_CARAPACE_VERITY_VERSION,
            ANNOTATION_CARAPACE_VERITY_HASH_TYPE,
            ANNOTATION_CARAPACE_VERITY_HASH,
        ] {
            assert!(
                name.starts_with(ANNOTATION_CARAPACE_VERITY_PREFIX),
                "{name}"
            );
        }
    }
}
