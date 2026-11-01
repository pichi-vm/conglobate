// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::Error;
use serde::{Deserialize, Serialize};

/// Image-root-relative path; a leading slash does not escape that namespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ImagePath(String);
impl TryFrom<String> for ImagePath {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() || value.contains('\0') || value.split('/').any(|part| part == "..") {
            return Err(Error::from(
                "image path must be nonempty without NUL or parent traversal",
            ));
        }
        Ok(Self(value))
    }
}
impl AsRef<str> for ImagePath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl From<ImagePath> for String {
    fn from(value: ImagePath) -> Self {
        value.0
    }
}
impl ImagePath {
    /// Canonical path for comparison; resolution still requires a confined root.
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
