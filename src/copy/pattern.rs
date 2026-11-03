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
pub(crate) struct PatternError;

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
pub(crate) struct SourcePattern {
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
    pub(crate) fn source(&self) -> &str {
        &self.source
    }
    /// Match an already context-relative candidate name. This grants no filesystem
    /// access and performs no traversal or symlink resolution.
    pub(crate) fn matches(&self, name: &OsStr) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    struct SmallGlob;
    impl SmallGlob {
        // Independent recursive definition, used only on inputs of at most four
        // tokens/runes. Production uses a forward state vector instead.
        fn matches(pattern: &[u8], name: &[u8]) -> bool {
            match pattern.split_first() {
                None => name.is_empty(),
                Some((b'*', rest)) => (0..=name.len()).any(|n| Self::matches(rest, &name[n..])),
                Some((token, rest)) => name.split_first().is_some_and(|(first, tail)| {
                    (*token == b'?' || token == first) && Self::matches(rest, tail)
                }),
            }
        }

        fn words(alphabet: &[u8], length: usize) -> Vec<Vec<u8>> {
            let mut words = vec![Vec::new()];
            for _ in 0..length {
                words = words
                    .into_iter()
                    .flat_map(|prefix| {
                        alphabet.iter().map(move |byte| {
                            let mut word = prefix.clone();
                            word.push(*byte);
                            word
                        })
                    })
                    .collect();
            }
            words
        }
    }

    #[test]
    fn short_globs_agree_with_independent_rune_sequence_definition() {
        let names: Vec<_> = (0..=4).flat_map(|n| SmallGlob::words(b"abXY", n)).collect();
        for length in 1..=4 {
            for pattern in SmallGlob::words(b"ab*?", length) {
                let text = std::str::from_utf8(&pattern).unwrap();
                let compiled = text.parse::<SourcePattern>().unwrap();
                for name in &names {
                    // X is one Unicode rune; Y is one invalid UTF-8 byte. Neither
                    // can match literal a/b, but each must consume exactly one ?.
                    let bytes: Vec<u8> = name
                        .iter()
                        .flat_map(|byte| match byte {
                            b'X' => "é".as_bytes().to_vec(),
                            b'Y' => vec![0xff],
                            byte => vec![*byte],
                        })
                        .collect();
                    assert_eq!(
                        compiled.matches(OsStr::from_bytes(&bytes)),
                        SmallGlob::matches(&pattern, name),
                        "pattern {text:?}, name {bytes:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn linux_filepath_patterns_cover_unicode_classes_and_literal_braces() {
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
            ("**", "dir/file", false),
            ("dir/?", "dir/é", true),
            ("dir/*", "dir/a/b", false),
            ("/dir/./file/", "dir/file", true),
            (".", "", true),
        ] {
            assert_eq!(
                pattern
                    .parse::<SourcePattern>()
                    .unwrap()
                    .matches(OsStr::new(name)),
                expected,
                "{pattern} {name}"
            );
        }
        assert!(
            "?".parse::<SourcePattern>()
                .unwrap()
                .matches(OsStr::from_bytes(b"\xff"))
        );
        assert!(
            "??".parse::<SourcePattern>()
                .unwrap()
                .matches(OsStr::from_bytes(b"\xff\xfe"))
        );
    }
}
