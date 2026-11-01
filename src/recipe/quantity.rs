// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::ValueError;
use std::num::NonZeroU64;
#[derive(Debug)]
pub(super) struct Quantity<'a> {
    pub(super) integer: NonZeroU64,
    pub(super) unit: &'a str,
}

impl<'a> TryFrom<&'a str> for Quantity<'a> {
    type Error = ValueError;

    fn try_from(text: &'a str) -> Result<Self, Self::Error> {
        let boundary = text
            .find(|c: char| !c.is_ascii_digit())
            .ok_or(ValueError::Syntax)?;
        if boundary == 0 {
            return Err(ValueError::Syntax);
        }
        let integer = text[..boundary]
            .parse::<u64>()
            .map_err(|_| ValueError::Overflow)?;
        Ok(Self {
            integer: NonZeroU64::new(integer).ok_or(ValueError::Syntax)?,
            unit: &text[boundary..],
        })
    }
}
