// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CommandTimeout, Error};
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exec {
    pub exec: String,
    #[serde(default)]
    pub network: bool,
    #[serde(default)]
    pub timeout: CommandTimeout,
}
impl Exec {
    pub fn validate(&self) -> Result<(), Error> {
        if self.exec.trim().is_empty() || self.exec.contains('\0') {
            return Err(Error::from("exec must be nonempty without NUL"));
        }
        Ok(())
    }
}
