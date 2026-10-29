// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Hexadecimal text for digests and salts, without an extra dependency.

const DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Lowercase hex text for `bytes`.
pub(crate) fn encode(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from(DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(DIGITS[usize::from(byte & 0xf)]));
    }
    text
}

/// Bytes for hex `text` in either case, or `None` when the text has an odd
/// length or a character that is not a hex digit.
pub(crate) fn decode(text: impl AsRef<[u8]>) -> Option<Vec<u8>> {
    fn digit(c: u8) -> Option<u8> {
        char::from(c).to_digit(16).map(|d| d as u8)
    }
    let (pairs, rest) = text.as_ref().as_chunks::<2>();
    if !rest.is_empty() {
        return None;
    }
    pairs
        .iter()
        .map(|&[high, low]| Some(digit(high)? << 4 | digit(low)?))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_byte() {
        let bytes: Vec<u8> = (0..=255).collect();
        let text = encode(&bytes);
        assert_eq!(text.len(), 512);
        assert!(text.starts_with("000102") && text.ends_with("fdfeff"));
        assert_eq!(decode(&text), Some(bytes.clone()));
        assert_eq!(decode(text.to_uppercase()), Some(bytes));
    }

    #[test]
    fn rejects_bad_text() {
        assert_eq!(decode(""), Some(Vec::new()));
        for text in ["0", "abc", "0g", "+1", " 1", "é0"] {
            assert_eq!(decode(text), None, "{text}");
        }
    }
}
