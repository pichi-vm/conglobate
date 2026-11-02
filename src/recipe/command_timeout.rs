// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{ValueError, quantity::Quantity};
use std::{str::FromStr, time::Duration};
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CommandTimeout(Duration);

impl CommandTimeout {
    pub fn duration(self) -> Duration {
        self.0
    }
}

impl Default for CommandTimeout {
    fn default() -> Self {
        Self(Duration::from_secs(300))
    }
}

impl FromStr for CommandTimeout {
    type Err = ValueError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let quantity = Quantity::try_from(text)?;
        let factor = match quantity.unit {
            "ms" => 1,
            "s" => 1000,
            "m" => 60 * 1000,
            "h" => 60 * 60 * 1000,
            _ => return Err(ValueError::Syntax),
        };
        let millis = quantity
            .integer
            .get()
            .checked_mul(factor)
            .ok_or(ValueError::Overflow)?;
        // Parsing validates the duration independently of the current clock.
        // PreparedStep::start checks deadline representability before admission.
        Ok(Self(Duration::from_millis(millis)))
    }
}
impl TryFrom<String> for CommandTimeout {
    type Error = ValueError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}
impl From<CommandTimeout> for String {
    fn from(value: CommandTimeout) -> Self {
        format!("{}ms", value.duration().as_millis())
    }
}
