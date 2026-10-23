// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! One explicit parsing policy for authored YAML inputs.
use serde::de::DeserializeOwned;

#[derive(Debug)]
pub struct YamlDocument<'a> {
    text: &'a str,
}

impl<'a> From<&'a str> for YamlDocument<'a> {
    fn from(text: &'a str) -> Self {
        Self { text }
    }
}

impl YamlDocument<'_> {
    pub(crate) fn decode<T: DeserializeOwned>(&self) -> Result<T, serde_saphyr::Error> {
        // The dependency exposes a non-exhaustive options struct, not a builder.
        let mut options = serde_saphyr::Options::default();
        options.strict_booleans = true;
        options.legacy_octal_numbers = false;
        options.duplicate_keys = serde_saphyr::DuplicateKeyPolicy::Error;
        serde_saphyr::from_str_with_options(self.text, options)
    }
}
