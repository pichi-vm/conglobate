// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use regex::{Captures, Regex};

use crate::Error;

#[derive(Debug, Clone)]
enum Part {
    Literal(String),
    Variable(String),
    Capture(String),
}

/// Parsed once, expanded only when an operation actually writes a value.
#[derive(Debug, Clone)]
pub(crate) struct Template(Vec<Part>);

impl Template {
    pub(crate) fn parse(text: &str, regex: Option<&Regex>) -> Result<Self, Error> {
        let mut parts = Vec::new();
        let mut chars = text.chars().peekable();
        let mut literal = String::new();
        while let Some(ch) = chars.next() {
            if ch != '$' {
                literal.push(ch);
                continue;
            }
            if chars.peek() == Some(&'$') {
                chars.next();
                literal.push('$');
                continue;
            }
            if !literal.is_empty() {
                parts.push(Part::Literal(std::mem::take(&mut literal)));
            }
            let mut name = String::new();
            if chars.peek() == Some(&'{') {
                chars.next();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(c) => name.push(c),
                        None => {
                            return Err(Error::Invalid("unterminated template reference".into()));
                        }
                    }
                }
            } else {
                while chars
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
                {
                    name.push(chars.next().unwrap());
                }
            }
            if name.is_empty() {
                return Err(Error::Invalid(
                    "literal dollars must be escaped as $$".into(),
                ));
            }
            let variable = name.starts_with(|c: char| c.is_ascii_uppercase())
                && name
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_');
            if variable {
                parts.push(Part::Variable(name));
                continue;
            }
            let Some(regex) = regex else {
                return Err(Error::Invalid(format!(
                    "capture {name:?} is not valid in set data"
                )));
            };
            let exists = if name.chars().all(|c| c.is_ascii_digit()) {
                name.parse::<usize>()
                    .is_ok_and(|i| i < regex.captures_len())
            } else {
                name.starts_with(|c: char| c.is_ascii_lowercase())
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                    && regex.capture_names().flatten().any(|n| n == name)
            };
            if !exists {
                return Err(Error::Invalid(format!(
                    "unknown or invalid capture {name:?}"
                )));
            }
            parts.push(Part::Capture(name));
        }
        if !literal.is_empty() {
            parts.push(Part::Literal(literal));
        }
        Ok(Self(parts))
    }

    pub(crate) fn expand(
        &self,
        carapace: Option<&str>,
        captures: Option<&Captures<'_>>,
    ) -> Result<String, Error> {
        let mut output = String::new();
        for part in &self.0 {
            match part {
                Part::Literal(s) => output.push_str(s),
                Part::Variable(name) => {
                    let value = match name.as_str() {
                        "CARAPACE" => carapace,
                        _ => None,
                    }
                    .ok_or_else(|| Error::Invalid(format!("undefined build variable {name}")))?;
                    output.push_str(value);
                }
                Part::Capture(name) => {
                    let captures = captures.ok_or_else(|| {
                        Error::Invalid("capture outside regex replacement".into())
                    })?;
                    let value = match name.parse::<usize>() {
                        Ok(i) => captures.get(i),
                        Err(_) => captures.name(name),
                    };
                    if let Some(value) = value {
                        output.push_str(value.as_str());
                    }
                }
            }
        }
        if output.contains('\0') {
            return Err(Error::Invalid(
                "DTB strings cannot contain embedded NULs".into(),
            ));
        }
        Ok(output)
    }
}
