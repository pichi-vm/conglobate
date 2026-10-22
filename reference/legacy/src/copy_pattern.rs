// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Docker COPY uses Go filepath.Match, not shell brace expansion or bytewise ?.
//! Match one path component using the documented Go grammar. A bounded dynamic
//! program avoids exponential backtracking and handles non-UTF-8 Unix names.

use crate::Error;
use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

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
pub(crate) struct Pattern(Vec<Token>);

impl Pattern {
    pub(crate) fn parse(pattern: &str) -> Result<Self, Error> {
        let mut chars = pattern.chars().peekable();
        let mut tokens = Vec::new();
        while let Some(ch) = chars.next() {
            tokens.push(match ch {
                '*' => Token::Star,
                '?' => Token::Any,
                '[' => {
                    let negated = chars.next_if_eq(&'^').is_some();
                    let mut ranges = Vec::new();
                    loop {
                        if !ranges.is_empty() && chars.next_if_eq(&']').is_some() {
                            break;
                        }
                        let low = class_char(&mut chars)?;
                        let high = if chars.next_if_eq(&'-').is_some() {
                            class_char(&mut chars)?
                        } else {
                            low
                        };
                        ranges.push((low, high));
                    }
                    Token::Class { negated, ranges }
                }
                other => {
                    let ch = if other == '\\' {
                        chars.next().ok_or_else(invalid)?
                    } else {
                        other
                    };
                    Token::Literal(ch.to_string().into_bytes())
                }
            });
        }
        Ok(Self(tokens))
    }

    pub(crate) fn matches(&self, name: &OsStr) -> bool {
        let name = name.as_bytes();
        let mut previous = vec![false; name.len() + 1];
        previous[0] = true;
        for token in &self.0 {
            let mut next = vec![false; previous.len()];
            for offset in 0..=name.len() {
                if matches!(token, Token::Star) {
                    next[offset] = previous[offset] || (offset > 0 && next[offset - 1]);
                } else if previous[offset] {
                    match token {
                        Token::Literal(bytes) if name[offset..].starts_with(bytes) => {
                            next[offset + bytes.len()] = true;
                        }
                        Token::Any | Token::Class { .. } if offset < name.len() => {
                            let (ch, width) = rune(&name[offset..]);
                            let accepted = match token {
                                Token::Class { negated, ranges } => {
                                    ranges.iter().any(|&(lo, hi)| lo <= ch && ch <= hi) != *negated
                                }
                                _ => true,
                            };
                            if accepted {
                                next[offset + width] = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
            previous = next;
        }
        previous[name.len()]
    }
}

fn class_char(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<char, Error> {
    match chars.next() {
        Some('\\') => chars.next().ok_or_else(invalid),
        Some('-' | ']') | None => Err(invalid()),
        Some(ch) => Ok(ch),
    }
}

fn rune(bytes: &[u8]) -> (char, usize) {
    for width in 1..=4.min(bytes.len()) {
        if let Ok(text) = std::str::from_utf8(&bytes[..width]) {
            return (text.chars().next().expect("nonempty slice"), width);
        }
    }
    ('\u{fffd}', 1)
}

fn invalid() -> Error {
    Error::Invalid("invalid Docker COPY filename pattern".into())
}

#[cfg(test)]
mod tests {
    use super::Pattern;
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    #[test]
    fn docker_filepath_grammar() {
        for (pattern, name, expected) in [
            ("?", "é", true),
            ("??", "é", false),
            ("[à-ê]", "é", true),
            ("[!a]", "!", true),
            ("[!a]", "b", false),
            ("[^a]", "b", true),
            ("*{a,b}", "xa", false),
            ("*{a,b}", "x{a,b}", true),
            ("arr[[]0].txt", "arr[0].txt", true),
            (r"a\*", "a*", true),
            ("[z-a]", "z", false),
            ("**", ".hidden", true),
        ] {
            assert_eq!(
                Pattern::parse(pattern).unwrap().matches(OsStr::new(name)),
                expected,
                "{pattern} {name}"
            );
        }
        for pattern in ["[", "[]", "[a-]", "\\", "[^]"] {
            assert!(Pattern::parse(pattern).is_err(), "{pattern}");
        }
        assert!(
            Pattern::parse("?")
                .unwrap()
                .matches(OsStr::from_bytes(b"\xff"))
        );
        assert!(
            Pattern::parse("??")
                .unwrap()
                .matches(OsStr::from_bytes(b"\xff\xfe"))
        );
    }
}
