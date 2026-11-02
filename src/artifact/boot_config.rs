// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{BootConfigError, boot_config_parser::BootConfigParser};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    str::FromStr,
};

/// Native Linux boot configuration, independent of a YAML build plan.
///
/// Keys are dot-separated. An empty vector is a bare key; a vector containing
/// an empty string is an explicitly empty value. Parent values can coexist
/// with child keys. Bare parents with children are implicit in Linux's tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BootConfig {
    values: BTreeMap<String, Vec<String>>,
    text: String,
}

impl BootConfig {
    pub(crate) const MAX_NODES: usize = 8192;

    pub fn values(&self) -> &BTreeMap<String, Vec<String>> {
        &self.values
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub(crate) fn check_key(key: &str) -> Result<(), BootConfigError> {
        if crate::plan::Key::new(key.to_owned()).is_none() {
            return Err(BootConfigError(format!(
                "invalid key or key length/depth overflow: {key:?}"
            )));
        }
        Ok(())
    }

    pub(crate) fn space(c: u8) -> bool {
        matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 11 | 12)
    }

    pub(crate) fn check_value(value: &str) -> Result<(), BootConfigError> {
        if !value
            .bytes()
            .all(|c| c.is_ascii_graphic() || Self::space(c))
        {
            return Err(BootConfigError(
                "value contains non-printable or non-ASCII bytes".into(),
            ));
        }
        Ok(())
    }

    fn encode(value: &str, out: &mut String) -> Result<(), BootConfigError> {
        Self::check_value(value)?;
        let quote = if !value.contains('"') {
            Some('"')
        } else if !value.contains('\'') {
            Some('\'')
        } else {
            None
        };
        if let Some(quote) = quote {
            out.push(quote);
            out.push_str(value);
            out.push(quote);
        } else if !value
            .bytes()
            .any(|c| matches!(c, b',' | b';' | b'\n' | b'#' | b'}'))
            && !value
                .as_bytes()
                .first()
                .is_some_and(|c| Self::space(*c) || matches!(c, b'\'' | b'"'))
            && !value.as_bytes().last().is_some_and(|c| Self::space(*c))
        {
            out.push_str(value);
        } else {
            return Err(BootConfigError("value requires quoting but contains both quote characters; Linux bootconfig has no escaping".into()));
        }
        Ok(())
    }
}

impl TryFrom<BTreeMap<String, Vec<String>>> for BootConfig {
    type Error = BootConfigError;

    fn try_from(mut values: BTreeMap<String, Vec<String>>) -> Result<Self, Self::Error> {
        let mut parents = BTreeSet::new();
        let mut nodes = BTreeSet::new();
        let mut count = 0usize;
        for (key, value) in &values {
            Self::check_key(key)?;
            for (i, _) in key.match_indices('.') {
                parents.insert(key[..i].to_owned());
                nodes.insert(&key[..i]);
            }
            nodes.insert(key.as_str());
            count = count.saturating_add(value.len());
        }
        if count.saturating_add(nodes.len()) > Self::MAX_NODES {
            return Err(BootConfigError("Linux node limit exceeded".into()));
        }
        values.retain(|key, value| !value.is_empty() || !parents.contains(key));
        let mut text = String::new();
        for (key, value) in &values {
            text.push_str(key);
            if !value.is_empty() {
                text.push_str(" = ");
                for (i, item) in value.iter().enumerate() {
                    if i != 0 {
                        text.push_str(", ");
                    }
                    Self::encode(item, &mut text)?;
                }
            }
            text.push_str(";\n");
        }
        Ok(Self { values, text })
    }
}

impl FromStr for BootConfig {
    type Err = BootConfigError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        BootConfigParser::parse(text)?.try_into()
    }
}

impl AsRef<str> for BootConfig {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for BootConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}
