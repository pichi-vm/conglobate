// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::ValueError;
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FilesystemLabel(String);

impl TryFrom<String> for FilesystemLabel {
    type Error = ValueError;

    fn try_from(label: String) -> Result<Self, Self::Error> {
        if label.len() > 16 || label.contains('\0') {
            return Err(ValueError::Label);
        }
        Ok(Self(label))
    }
}

impl AsRef<str> for FilesystemLabel {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<FilesystemLabel> for String {
    fn from(value: FilesystemLabel) -> Self {
        value.0
    }
}
