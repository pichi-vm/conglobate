// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Parsed, single-pass transformation templates. Shell commands do not use these.
#[derive(Debug, thiserror::Error)]
#[error("invalid transformation template: {0}")]
pub struct TemplateError(&'static str);

#[derive(Debug)]
enum Part {
    Literal(String),
    Build(String),
    Capture(String),
}

#[derive(Debug)]
pub struct Template {
    parts: Vec<Part>,
}

impl Template {
    pub fn set(source: &str) -> Result<Self, TemplateError> {
        Self::parse(source, false)
    }

    pub fn replacement(source: &str) -> Result<Self, TemplateError> {
        Self::parse(source, true)
    }

    fn parse(source: &str, captures: bool) -> Result<Self, TemplateError> {
        if source.contains('\0') {
            return Err(TemplateError("NUL is not a string value"));
        }
        let mut parts = Vec::new();
        let mut rest = source;
        while let Some(start) = rest.find('$') {
            if start > 0 {
                parts.push(Part::Literal(rest[..start].to_owned()));
            }
            rest = &rest[start + 1..];
            if let Some(tail) = rest.strip_prefix('$') {
                parts.push(Part::Literal("$".to_owned()));
                rest = tail;
                continue;
            }
            let name;
            if let Some(tail) = rest.strip_prefix('{') {
                let end = tail.find('}').ok_or(TemplateError("unclosed reference"))?;
                name = &tail[..end];
                rest = &tail[end + 1..];
            } else {
                let end = rest
                    .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
                    .unwrap_or(rest.len());
                name = &rest[..end];
                rest = &rest[end..];
            }
            if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                return Err(TemplateError("expected an ASCII reference name"));
            }
            let build = name.bytes().any(|b| b.is_ascii_uppercase())
                && name
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_');
            if build {
                parts.push(Part::Build(name.to_owned()));
            } else if captures {
                parts.push(Part::Capture(name.to_owned()));
            } else {
                return Err(TemplateError("set values cannot reference regex captures"));
            }
        }
        if !rest.is_empty() {
            parts.push(Part::Literal(rest.to_owned()));
        }
        Ok(Self { parts })
    }

    /// Evaluation is deferred until a set executes or an actual match expands.
    /// Unmatched or unknown regex captures expand to empty, as in regex::Captures.
    pub fn expand(
        &self,
        carapace: Option<&str>,
        captures: Option<&regex::Captures<'_>>,
    ) -> Result<String, TemplateError> {
        let mut result = String::new();
        for part in &self.parts {
            match part {
                Part::Literal(value) => result.push_str(value),
                Part::Build(name) => {
                    let value = match name.as_str() {
                        "CARAPACE" => carapace,
                        _ => None,
                    }
                    .ok_or(TemplateError("undefined build variable"))?;
                    result.push_str(value);
                }
                Part::Capture(name) => {
                    let captures = captures.ok_or(TemplateError("missing capture context"))?;
                    let value = match name.parse::<usize>() {
                        Ok(index) => captures.get(index),
                        Err(_) => captures.name(name),
                    };
                    if let Some(value) = value {
                        result.push_str(value.as_str());
                    }
                }
            }
        }
        if result.contains('\0') {
            return Err(TemplateError("expanded string contains NUL"));
        }
        Ok(result)
    }
}
