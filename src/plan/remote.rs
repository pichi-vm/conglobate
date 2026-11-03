// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use oci_spec::distribution::Reference;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{cmp::Ordering, fmt};

/// A fully qualified image with an explicit registry, repository and selector.
/// Uses the OCI distribution reference grammar; IPv6-literal registries are unsupported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote(Reference);

impl Remote {
    pub fn new(value: String) -> Option<Self> {
        let (registry, repository) = value.split_once('/')?;
        // Require a tag or digest rather than accepting the parser's latest default.
        if !repository.contains('@') && !repository.rsplit('/').next()?.contains(':') {
            return None;
        }
        let parsed = value.parse::<Reference>().ok()?;
        if parsed.registry() != registry && parsed.resolve_registry() != registry {
            return None;
        }
        Some(Self(parsed))
    }
}

impl AsRef<Reference> for Remote {
    fn as_ref(&self) -> &Reference {
        &self.0
    }
}

impl Ord for Remote {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.0.registry(),
            self.0.repository(),
            self.0.tag(),
            self.0.digest(),
        )
            .cmp(&(
                other.0.registry(),
                other.0.repository(),
                other.0.tag(),
                other.0.digest(),
            ))
    }
}

impl PartialOrd for Remote {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Remote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl Serialize for Remote {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}
impl<'de> Deserialize<'de> for Remote {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).ok_or_else(|| {
            serde::de::Error::custom("expected an explicit registry, repository, and tag or digest")
        })
    }
}
