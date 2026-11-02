// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ImagePath(String);

impl ImagePath {
    pub fn new(value: String) -> Option<Self> {
        (!value.is_empty() && !value.contains('\0') && !value.split('/').any(|part| part == ".."))
            .then_some(Self(value))
    }
}
impl std::ops::Deref for ImagePath {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<ImagePath> for String {
    fn from(value: ImagePath) -> Self {
        value.0
    }
}
impl AsRef<str> for ImagePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl std::borrow::Borrow<str> for ImagePath {
    fn borrow(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for ImagePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl<'de> Deserialize<'de> for ImagePath {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::new(String::deserialize(deserializer)?)
            .ok_or_else(|| serde::de::Error::custom("expected a nonempty confined image path"))
    }
}
impl ImagePath {
    /// Compare paths within their selected namespace; resolve through a confined root.
    pub fn normalized(&self) -> String {
        format!(
            "/{}",
            self.0
                .split('/')
                .filter(|part| !part.is_empty() && *part != ".")
                .collect::<Vec<_>>()
                .join("/")
        )
    }
}
