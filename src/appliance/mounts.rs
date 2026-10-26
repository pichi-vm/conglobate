// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Fixed appliance host paths and image-hardcoded execution inputs.
//! Tags and mount wiring live in the image units, never in CBOR.

use crate::isolation::{IdMappings, IdRange};
use std::path::Path;

pub(crate) const REPOSITORY: &str = "/run/pichi/repo";
pub(crate) const CACHE: &str = "/run/pichi/cache";
pub(crate) const OUTPUT: &str = "/run/pichi/output";
pub(crate) const NETWORK_NAMESPACE: &str = "/run/pichi/netns";
pub(crate) const RESOLV_CONF: &str = "/run/pichi/resolv.conf";

/// Image-allocated parent identity ranges (not CBOR, not argv).
pub(crate) fn id_mappings() -> IdMappings {
    // 65536 ids starting at 100000 — matches the common subordinate range shape.
    let uid = IdRange::new(100_000, 65_536).expect("appliance uid map constant");
    let gid = IdRange::new(100_000, 65_536).expect("appliance gid map constant");
    IdMappings::new(uid, gid)
}

pub(crate) fn repository() -> &'static Path {
    Path::new(REPOSITORY)
}
pub(crate) fn cache() -> &'static Path {
    Path::new(CACHE)
}
pub(crate) fn output() -> &'static Path {
    Path::new(OUTPUT)
}
pub(crate) fn network_namespace() -> &'static Path {
    Path::new(NETWORK_NAMESPACE)
}
pub(crate) fn resolv_conf() -> &'static Path {
    Path::new(RESOLV_CONF)
}
