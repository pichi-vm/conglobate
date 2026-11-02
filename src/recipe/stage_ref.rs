// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::Error;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct StageRef(String);
impl FromStr for StageRef {
    type Err = Error;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.is_empty()
            || matches!(text, "." | "..")
            || !text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        {
            return Err(Error::from(
                "expected a bare local name (letters, digits, _, . or -)",
            ));
        }
        Ok(Self(text.into()))
    }
}
impl AsRef<str> for StageRef {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for StageRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<String> for StageRef {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
impl From<StageRef> for String {
    fn from(value: StageRef) -> Self {
        value.as_ref().to_owned()
    }
}
