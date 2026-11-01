// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::Error;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Remote {
    text: String,
}
impl AsRef<str> for Remote {
    fn as_ref(&self) -> &str {
        &self.text
    }
}
impl FromStr for Remote {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || {
            Error::from(
                "remote reference requires a valid explicit registry host and OCI repository",
            )
        };
        let (registry, _) = text.split_once('/').ok_or_else(invalid)?;
        let (host, port) = if let Some(ipv6) = registry.strip_prefix('[') {
            let (address, suffix) = ipv6.split_once(']').ok_or_else(invalid)?;
            address
                .parse::<std::net::Ipv6Addr>()
                .map_err(|_| invalid())?;
            (
                address,
                if suffix.is_empty() {
                    None
                } else {
                    Some(suffix.strip_prefix(':').ok_or_else(invalid)?)
                },
            )
        } else {
            match registry.rsplit_once(':') {
                Some((host, port)) => (host, Some(port)),
                None => (registry, None),
            }
        };
        let ipv6 = registry.starts_with('[');
        if (!ipv6
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
            return Err(invalid());
        }
        let parsed: crate::artifact::Reference = text
            .parse::<crate::artifact::Reference>()
            .map_err(|error| Error(error.to_string()))?;
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
            return Err(invalid());
        }
        Ok(Self { text: text.into() })
    }
}

impl TryFrom<String> for Remote {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
impl From<Remote> for String {
    fn from(value: Remote) -> Self {
        value.as_ref().to_owned()
    }
}
