// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{BootConfig, BootConfigError};
use std::collections::{BTreeMap, BTreeSet};

/// Bounded parser for Linux's documented syntax. No shell or YAML decoding.
#[derive(Debug)]
pub(super) struct BootConfigParser<'a> {
    text: &'a str,
    at: usize,
    values: BTreeMap<String, Vec<String>>,
    keys: BTreeSet<String>,
    allocated_values: usize,
}

impl<'a> BootConfigParser<'a> {
    pub(super) fn parse(text: &'a str) -> Result<BTreeMap<String, Vec<String>>, BootConfigError> {
        if text.len() > BootConfig::MAX_SIZE || text.contains('\0') {
            return Err(BootConfigError(
                "native text exceeds size limit or contains NUL".into(),
            ));
        }
        let mut parser = Self {
            text,
            at: 0,
            values: BTreeMap::new(),
            keys: BTreeSet::new(),
            allocated_values: 0,
        };
        parser.scope("")?;
        if !text.is_empty() && parser.values.is_empty() {
            return Err(parser.error("no keys"));
        }
        Ok(parser.values)
    }

    fn error(&self, reason: &str) -> BootConfigError {
        BootConfigError(format!("{reason} at byte {}", self.at))
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn spaces(&mut self, newline: bool) {
        while self
            .peek()
            .is_some_and(|c| BootConfig::space(c) && (newline || c != b'\n'))
        {
            self.at += 1;
        }
    }

    fn comment(&mut self) {
        while self.peek().is_some_and(|c| c != b'\n') {
            self.at += 1;
        }
    }

    fn scope(&mut self, prefix: &str) -> Result<(), BootConfigError> {
        loop {
            self.spaces(true);
            match self.peek() {
                Some(b';') => {
                    self.at += 1;
                    continue;
                }
                Some(b'#') => {
                    self.comment();
                    continue;
                }
                Some(b'}') if !prefix.is_empty() => {
                    self.at += 1;
                    return Ok(());
                }
                None if prefix.is_empty() => return Ok(()),
                None => return Err(self.error("unclosed key group")),
                _ => {}
            }
            let start = self.at;
            while self
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
            {
                self.at += 1;
            }
            if start == self.at {
                return Err(self.error("expected key"));
            }
            let word = &self.text[start..self.at];
            let key = if prefix.is_empty() {
                word.to_owned()
            } else {
                format!("{prefix}.{word}")
            };
            BootConfig::check_key(&key)?;
            for (end, _) in key.match_indices('.') {
                self.keys.insert(key[..end].to_owned());
            }
            self.keys.insert(key.clone());
            self.check_nodes()?;
            self.spaces(false);
            let op = self
                .peek()
                .ok_or_else(|| self.error("bare key requires a delimiter"))?;
            self.values.entry(key.clone()).or_default();
            match op {
                b'{' => {
                    self.at += 1;
                    self.scope(&key)?;
                }
                b'=' | b':' | b'+' => {
                    self.at += 1;
                    if op != b'=' {
                        if self.peek() != Some(b'=') {
                            return Err(self.error("expected := or +="));
                        }
                        self.at += 1;
                    }
                    let values = self.array()?;
                    let current = self.values.get_mut(&key).expect("inserted key");
                    if op == b'=' && !current.is_empty() {
                        return Err(self.error("duplicate value; use := or +="));
                    }
                    if op == b'+' {
                        self.allocated_values += values.len();
                        current.extend(values);
                    } else {
                        // := reuses the first value node; old array nodes stay
                        // allocated in Linux's bounded parser arena.
                        self.allocated_values += values.len() - usize::from(!current.is_empty());
                        *current = values;
                    }
                    self.check_nodes()?;
                }
                b';' | b'\n' => self.at += 1,
                b'#' => self.comment(),
                b'}' => {}
                _ => return Err(self.error("expected assignment, group, or delimiter")),
            }
        }
    }

    fn check_nodes(&self) -> Result<(), BootConfigError> {
        if self.keys.len() + self.allocated_values > BootConfig::MAX_NODES {
            Err(self.error("Linux node limit exceeded"))
        } else {
            Ok(())
        }
    }

    fn array(&mut self) -> Result<Vec<String>, BootConfigError> {
        let mut values = vec![self.value()?];
        while self.peek() == Some(b',') {
            self.at += 1;
            loop {
                self.spaces(true);
                if self.peek() != Some(b'#') {
                    break;
                }
                self.comment();
            }
            values.push(self.value()?);
            if values.len() > BootConfig::MAX_NODES {
                return Err(self.error("Linux node limit exceeded"));
            }
        }
        Ok(values)
    }

    fn value(&mut self) -> Result<String, BootConfigError> {
        self.spaces(false);
        let value = if let Some(quote @ (b'\'' | b'"')) = self.peek() {
            self.at += 1;
            let start = self.at;
            while self.peek().is_some_and(|c| c != quote) {
                self.at += 1;
            }
            if self.peek().is_none() {
                return Err(self.error("unterminated quoted value"));
            }
            let value = &self.text[start..self.at];
            self.at += 1;
            self.spaces(false);
            if self
                .peek()
                .is_some_and(|c| !matches!(c, b',' | b';' | b'\n' | b'#' | b'}'))
            {
                return Err(self.error("text after quoted value"));
            }
            value
        } else {
            let start = self.at;
            while self
                .peek()
                .is_some_and(|c| !matches!(c, b',' | b';' | b'\n' | b'#' | b'}'))
            {
                self.at += 1;
            }
            let value = &self.text[start..self.at];
            // Linux preserves trailing whitespace at EOF, unlike before a delimiter.
            if self.peek().is_none() {
                value
            } else {
                value.trim_end_matches(|c: char| c.is_ascii() && BootConfig::space(c as u8))
            }
        };
        BootConfig::check_value(value)?;
        Ok(value.to_owned())
    }
}
