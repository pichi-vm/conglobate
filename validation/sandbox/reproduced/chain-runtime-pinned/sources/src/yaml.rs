// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! One parsing policy for all authored files.

pub(crate) fn from_str<T: serde::de::DeserializeOwned>(
    text: &str,
) -> Result<T, serde_saphyr::Error> {
    let mut options = serde_saphyr::Options::default();
    // Preserve YAML 1.2 scalar behavior: y/n and on/off are strings.
    options.strict_booleans = true;
    options.legacy_octal_numbers = false;
    serde_saphyr::from_str_with_options(text, options)
}
