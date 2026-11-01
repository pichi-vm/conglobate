// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

// Binary-local contract tests; no production library API.
#[path = "../tests/unit/anchors.rs"]
mod anchors;
#[path = "../tests/unit/boot_configs.rs"]
mod boot_configs;
#[path = "../tests/unit/cache.rs"]
mod cache;
#[path = "../tests/unit/contexts.rs"]
mod contexts;
#[path = "../tests/unit/copy_modes.rs"]
mod copy_modes;
#[path = "../tests/unit/copy_patterns.rs"]
mod copy_patterns;
#[path = "../tests/unit/graphs.rs"]
mod graphs;
#[path = "../tests/unit/identities.rs"]
mod identities;
#[path = "../tests/unit/locks.rs"]
mod locks;
#[path = "../tests/unit/plans.rs"]
mod plans;
#[path = "../tests/unit/processes.rs"]
mod processes;
#[path = "../tests/unit/requests.rs"]
mod requests;
#[path = "../tests/unit/supervision.rs"]
mod supervision;
#[path = "../tests/unit/values.rs"]
mod values;

/// Authentic roots for cache metadata fixtures; production only seals snapshots.
pub(crate) trait ScuteFixture {
    fn scute_files(&self, parent: [u8; 32]) -> (devmap_verity::Header, Vec<u8>, [u8; 32]);
    fn scute_root(&self, parent: [u8; 32]) -> [u8; 32] {
        self.scute_files(parent).2
    }
}
impl ScuteFixture for [u8] {
    fn scute_files(&self, parent: [u8; 32]) -> (devmap_verity::Header, Vec<u8>, [u8; 32]) {
        use devmap_core::{BlockSize, Geometry};
        use devmap_verity::{Algorithm, Format as _, HashType, Header, Parameters, Salt};
        use std::{io::Cursor, num::NonZero};
        let block = BlockSize::from_bytes(NonZero::new(4096).unwrap()).unwrap();
        let mut data = self.to_vec();
        data.resize(data.len().div_ceil(4096) * 4096, 0);
        let header = Header {
            uuid: [0; 16],
            parameters: Parameters {
                algorithm: Algorithm::Sha256,
                hash_type: HashType::Normal,
                salt: Salt::new(&parent).unwrap(),
                data: Geometry {
                    size: block,
                    count: NonZero::new(data.len() as u64 / 4096).unwrap(),
                },
                hash: block,
            },
        };
        let mut tree = Cursor::new(Vec::new());
        let root = header.format(data.as_slice(), &mut tree).unwrap();
        (header, tree.into_inner(), root.as_ref().try_into().unwrap())
    }
}
