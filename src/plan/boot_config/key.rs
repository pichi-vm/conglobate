// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct Key(String);

impl Key {
    pub fn new(value: String) -> Option<Self> {
        (value.len() < 256
            && value.split('.').count() <= 16
            && value.split('.').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
            }))
        .then_some(Self(value))
    }
}
impl std::ops::Deref for Key {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<Key> for String {
    fn from(value: Key) -> Self {
        value.0
    }
}
impl AsRef<str> for Key {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl std::borrow::Borrow<str> for Key {
    fn borrow(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<'de> Deserialize<'de> for Key {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?).ok_or_else(|| {
            serde::de::Error::custom(
                "expected a boot configuration key with nonempty dot-separated names",
            )
        })
    }
}
