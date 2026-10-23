// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Go filepath.Match grammar for Linux COPY source selection.
use std::{
    ffi::OsStr,
    iter::Peekable,
    os::unix::ffi::OsStrExt,
    str::{Chars, FromStr},
};

#[derive(Debug, thiserror::Error)]
#[error("invalid COPY source or filename pattern")]
pub struct PatternError;

#[derive(Debug)]
enum Token {
    Literal(Vec<u8>),
    Star,
    Any,
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}
#[derive(Debug)]
pub struct SourcePattern {
    source: String,
    tokens: Vec<Token>,
}
impl FromStr for SourcePattern {
    type Err = PatternError;
    fn from_str(source: &str) -> Result<Self, Self::Err> {
        if source.is_empty() || source.contains('\0') || source.split('/').any(|part| part == "..")
        {
            return Err(PatternError);
        }
        // Docker treats leading/trailing separators as context-relative selection.
        let normalized = source
            .trim_matches('/')
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .collect::<Vec<_>>()
            .join("/");
        let tokens = PatternParser {
            chars: normalized.chars().peekable(),
        }
        .parse()?;
        Ok(Self {
            source: source.to_owned(),
            tokens,
        })
    }
}
impl SourcePattern {
    pub fn source(&self) -> &str {
        &self.source
    }
    /// Match an already context-relative candidate name. This grants no filesystem
    /// access and performs no traversal or symlink resolution.
    pub fn matches(&self, name: &OsStr) -> bool {
        let bytes = name.as_bytes();
        let mut reachable = vec![false; bytes.len() + 1];
        reachable[0] = true;
        let mut next = vec![false; reachable.len()];
        for token in &self.tokens {
            next.fill(false);
            for offset in 0..=bytes.len() {
                if matches!(token, Token::Star) {
                    next[offset] = reachable[offset]
                        || (offset > 0 && bytes[offset - 1] != b'/' && next[offset - 1]);
                    continue;
                }
                if !reachable[offset] {
                    continue;
                }
                match token {
                    Token::Literal(literal) if bytes[offset..].starts_with(literal) => {
                        next[offset + literal.len()] = true
                    }
                    Token::Any if offset < bytes.len() && bytes[offset] != b'/' => {
                        next[offset + Rune::decode(&bytes[offset..]).width] = true;
                    }
                    Token::Class { negated, ranges } if offset < bytes.len() => {
                        let rune = Rune::decode(&bytes[offset..]);
                        if ranges
                            .iter()
                            .any(|&(low, high)| low <= rune.value && rune.value <= high)
                            != *negated
                        {
                            next[offset + rune.width] = true;
                        }
                    }
                    _ => {}
                }
            }
            std::mem::swap(&mut reachable, &mut next);
        }
        reachable[bytes.len()]
    }
}

#[derive(Debug)]
struct PatternParser<'a> {
    chars: Peekable<Chars<'a>>,
}
impl PatternParser<'_> {
    fn parse(mut self) -> Result<Vec<Token>, PatternError> {
        let mut tokens = Vec::new();
        while let Some(ch) = self.chars.next() {
            let token = match ch {
                '*' => Token::Star,
                '?' => Token::Any,
                '[' => self.class()?,
                '\\' => Token::Literal(
                    self.chars
                        .next()
                        .ok_or(PatternError)?
                        .to_string()
                        .into_bytes(),
                ),
                literal => Token::Literal(literal.to_string().into_bytes()),
            };
            tokens.push(token);
        }
        Ok(tokens)
    }
    fn class(&mut self) -> Result<Token, PatternError> {
        let negated = self.chars.next_if_eq(&'^').is_some();
        let mut ranges = Vec::new();
        loop {
            if !ranges.is_empty() && self.chars.next_if_eq(&']').is_some() {
                break;
            }
            let low = self.class_character()?;
            let high = if self.chars.next_if_eq(&'-').is_some() {
                self.class_character()?
            } else {
                low
            };
            ranges.push((low, high));
        }
        Ok(Token::Class { negated, ranges })
    }
    fn class_character(&mut self) -> Result<char, PatternError> {
        match self.chars.next() {
            Some('\\') => self.chars.next().ok_or(PatternError),
            Some(']' | '-') | None => Err(PatternError),
            Some(ch) => Ok(ch),
        }
    }
}

#[derive(Debug)]
struct Rune {
    value: char,
    width: usize,
}
impl Rune {
    fn decode(bytes: &[u8]) -> Self {
        for width in 1..=4.min(bytes.len()) {
            if let Ok(text) = std::str::from_utf8(&bytes[..width]) {
                return Self {
                    value: text.chars().next().expect("nonempty UTF-8"),
                    width,
                };
            }
        }
        Self {
            value: '\u{fffd}',
            width: 1,
        }
    }
}
