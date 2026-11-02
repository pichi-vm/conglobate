// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// Invalid authored recipe, without execution or filesystem authority.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("invalid recipe: {0}")]
pub struct Error(pub String);
impl From<&str> for Error {
    fn from(value: &str) -> Self {
        Self(value.into())
    }
}
impl From<String> for Error {
    fn from(value: String) -> Self {
        Self(value)
    }
}
