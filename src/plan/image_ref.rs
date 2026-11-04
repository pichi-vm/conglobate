// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Remote, StageRef};
use serde::{Deserialize, Serialize};

/// Disjoint local-stage and qualified-registry namespaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, try_from = "String")]
pub enum ImageRef {
    Local(StageRef),
    Remote(Remote),
}

impl TryFrom<String> for ImageRef {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if let Some(local) = StageRef::new(value.clone()) {
            return Ok(Self::Local(local));
        }
        let remote = Remote::new(value).ok_or("expected a stage or fully qualified image")?;
        if remote
            .registry()
            .split(':')
            .next()
            .is_some_and(|host| host.eq_ignore_ascii_case("localhost"))
        {
            return Err("localhost images require !bootstrap on a tool use");
        }
        Ok(Self::Remote(remote))
    }
}
