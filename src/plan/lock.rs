// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Separate remote-reference lock. Local task names never appear here.
use super::{Error, Remote, decode_yaml, nonnull};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
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
impl Serialize for IndexPin {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&format!("sha256:{}", hex::encode(self.0)))
    }
}
#[derive(Debug, Deserialize, Serialize)]
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
/// Remote references pinned only to OCI index digests (DESIGN §4).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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
    /// Build a lock from validated remote references and index digests.
    pub fn new(images: BTreeMap<Remote, IndexPin>) -> Self {
        Self { images }
    }

    /// Write version 1 lock YAML, with references in sorted order.
    pub fn to_yaml(&self) -> Result<String, Error> {
        #[derive(Serialize)]
        struct Document<'a> {
            version: u64,
            images: BTreeMap<&'a str, Pin>,
        }
        let document = Document {
            version: 1,
            images: self
                .images
                .iter()
                .map(|(remote, &index)| (remote.as_ref(), Pin { index }))
                .collect(),
        };
        serde_yaml::to_string(&document).map_err(Error::invalid)
    }

    pub fn parse(text: &str) -> Result<Self, Error> {
        decode_yaml(text)
    }
    pub fn images(&self) -> &BTreeMap<Remote, IndexPin> {
        &self.images
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructed_locks_round_trip() {
        let images = [
            ("registry.example.com/z:1", "ab"),
            ("registry.example.com/a:2", "cd"),
        ]
        .into_iter()
        .map(|(remote, byte)| {
            (
                remote.parse().unwrap(),
                format!("sha256:{}", byte.repeat(32)).parse().unwrap(),
            )
        })
        .collect();
        for lock in [Lock::new(images), Lock::new(BTreeMap::new())] {
            let yaml = lock.to_yaml().unwrap();
            assert_eq!(Lock::parse(&yaml).unwrap(), lock);
            let document: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
            assert_eq!(document["version"], 1);
            for pin in document["images"].as_mapping().unwrap().values() {
                assert_eq!(pin.as_mapping().unwrap().len(), 1);
                assert!(pin["index"].as_str().unwrap().starts_with("sha256:"));
            }
        }
    }

    #[test]
    fn existing_fixture_round_trips() {
        let lock = Lock::parse(include_str!("../../tests/distro/context/lock.yaml")).unwrap();
        assert_eq!(Lock::parse(&lock.to_yaml().unwrap()).unwrap(), lock);
    }
}
