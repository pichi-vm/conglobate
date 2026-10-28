// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Separate remote-reference lock. Local task names never appear here.
use super::{Error, Remote, decode_yaml, nonnull};
use serde::{Deserialize, Deserializer, de};
use std::{collections::BTreeMap, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexPin([u8; 32]);
impl IndexPin {
    pub fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl FromStr for IndexPin {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Error> {
        let digest: crate::artifact::Digest = text.parse().map_err(Error::invalid)?;
        Ok(Self(digest.as_sha256_array()))
    }
}
impl<'de> Deserialize<'de> for IndexPin {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?.parse().map_err(de::Error::custom)
    }
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pin {
    index: IndexPin,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLock {
    version: u64,
    #[serde(deserialize_with = "nonnull")]
    images: BTreeMap<Remote, Pin>,
}
#[derive(Debug, Deserialize)]
#[serde(try_from = "RawLock")]
pub struct Lock {
    images: BTreeMap<Remote, IndexPin>,
}
impl TryFrom<RawLock> for Lock {
    type Error = Error;
    fn try_from(raw: RawLock) -> Result<Self, Error> {
        if raw.version != 1 {
            return Err(Error::invalid("unsupported lock version"));
        }
        Ok(Self {
            images: raw.images.into_iter().map(|(r, p)| (r, p.index)).collect(),
        })
    }
}
impl Lock {
    pub fn parse(text: &str) -> Result<Self, Error> {
        decode_yaml(text)
    }
    pub fn images(&self) -> &BTreeMap<Remote, IndexPin> {
        &self.images
    }
}
