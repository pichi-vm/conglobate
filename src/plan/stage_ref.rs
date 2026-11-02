// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct StageRef(String);

impl StageRef {
    pub fn new(value: String) -> Option<Self> {
        (!value.is_empty()
            && !matches!(value.as_str(), "." | "..")
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)))
        .then_some(Self(value))
    }
}
impl std::ops::Deref for StageRef {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<StageRef> for String {
    fn from(value: StageRef) -> Self {
        value.0
    }
}
impl AsRef<str> for StageRef {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl std::borrow::Borrow<str> for StageRef {
    fn borrow(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for StageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<'de> Deserialize<'de> for StageRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?)
            .ok_or_else(|| serde::de::Error::custom("expected a bare local stage name"))
    }
}
