// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! One explicit parsing policy for authored YAML inputs.
use serde::de::DeserializeOwned;

#[derive(Debug)]
pub(crate) struct YamlDocument<'a> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decoding_retains_the_scalar_byte_budget() {
        // Fixed, flat input: exercise the production default without recursive
        // expansion or a test-only parser configuration. Drop the decoded copy
        // before testing rejection to bound peak fixture storage.
        const LIMIT: usize = 64 * 1024 * 1024;
        let mut text = "x".repeat(LIMIT);
        let decoded = YamlDocument::from(text.as_str())
            .decode::<String>()
            .unwrap();
        assert_eq!(decoded, text);
        drop(decoded);
        text.push('x');
        let error = YamlDocument::from(text.as_str())
            .decode::<String>()
            .unwrap_err();
        let mut cause = &error;
        while let serde_saphyr::Error::WithSnippet { error, .. } = cause {
            cause = error;
        }
        assert!(
            matches!(cause, serde_saphyr::Error::Budget {
                breach: serde_saphyr::budget::BudgetBreach::ScalarBytes { total_scalar_bytes },
                ..
            } if *total_scalar_bytes == LIMIT + 1),
            "{error}"
        );
    }

    #[test]
    fn decoding_retains_the_alias_ratio_budget() {
        // Flat scalar references keep both input and possible output tiny.
        // This exercises ratio enforcement without recursive expansion.
        let control = "[&item 0, *item]";
        assert_eq!(
            YamlDocument::from(control).decode::<Vec<u8>>().unwrap(),
            vec![0, 0]
        );
        let repeated = format!("[&item 0{}]", ", *item".repeat(101));
        let error = YamlDocument::from(repeated.as_str())
            .decode::<Vec<u8>>()
            .unwrap_err();
        let message = error.to_string().to_lowercase();
        assert!(
            message.contains("alias") && message.contains("ratio"),
            "{error}"
        );
    }

    #[test]
    fn decoding_retains_the_parser_depth_budget() {
        // Small input exercises depth enforcement without resource exhaustion.
        let shallow = format!("{}0{}", "[".repeat(8), "]".repeat(8));
        let deep = format!("{}0{}", "[".repeat(80), "]".repeat(80));
        assert!(
            YamlDocument::from(shallow.as_str())
                .decode::<serde_json::Value>()
                .is_ok()
        );
        let error = YamlDocument::from(deep.as_str())
            .decode::<serde_json::Value>()
            .unwrap_err();
        assert!(
            error.to_string().to_lowercase().contains("depth"),
            "{error}"
        );
    }
}
