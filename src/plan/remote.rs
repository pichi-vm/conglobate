// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::artifact::{Reference, ReferenceKind};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A fully qualified image with an explicit registry, repository and selector.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Remote {
    registry: String,
    repository: String,
    selector: ReferenceKind,
}

impl Remote {
    pub fn new(value: String) -> Option<Self> {
        let (registry, repository) = value.split_once('/')?;
        // Require a tag or digest rather than accepting the parser's latest default.
        if !repository.contains('@') && !repository.rsplit('/').next()?.contains(':') {
            return None;
        }
        let (host, port) = if let Some(ipv6) = registry.strip_prefix('[') {
            let (address, suffix) = ipv6.split_once(']')?;
            address.parse::<std::net::Ipv6Addr>().ok()?;
            (
                address,
                if suffix.is_empty() {
                    None
                } else {
                    Some(suffix.strip_prefix(':')?)
                },
            )
        } else {
            match registry.rsplit_once(':') {
                Some((host, port)) => (host, Some(port)),
                None => (registry, None),
            }
        };
        if (!registry.starts_with('[')
            && (!(host == "localhost" || host.contains('.') || port.is_some())
                || host.split('.').any(|part| {
                    part.is_empty()
                        || !part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                        || part.starts_with('-')
                        || part.ends_with('-')
                })))
            || port.is_some_and(|p| {
                !p.bytes().all(|b| b.is_ascii_digit())
                    || p.parse::<u16>().ok().filter(|p| *p > 0).is_none()
            })
        {
            return None;
        }
        let parsed = value.parse::<Reference>().ok()?;
        if parsed.registry != registry
            || parsed.repo.split('/').any(|part| {
                part.is_empty()
                    || !part.bytes().all(|b| {
                        b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b)
                    })
                    || !part.as_bytes()[0].is_ascii_alphanumeric()
                    || !part.as_bytes()[part.len() - 1].is_ascii_alphanumeric()
            })
        {
            return None;
        }
        Some(Self {
            registry: parsed.registry,
            repository: parsed.repo,
            selector: parsed.kind,
        })
    }

    pub fn registry(&self) -> &str {
        &self.registry
    }
    pub fn repository(&self) -> &str {
        &self.repository
    }
    pub fn selector(&self) -> &ReferenceKind {
        &self.selector
    }
}
impl fmt::Display for Remote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.registry, self.repository)?;
        match &self.selector {
            ReferenceKind::Tag(tag) => write!(f, ":{tag}"),
            ReferenceKind::Digest(digest) => write!(f, "@{digest}"),
        }
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
