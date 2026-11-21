// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Content-addressable digests. See crate-level docs for D-01.

use super::Sha1;
use std::{convert::Infallible, fmt, str::FromStr};
use thiserror::Error;

/// Typed digest, with SHA-1 uninhabited unless its byte storage is enabled.
///
/// ```compile_fail
/// use conglobate::artifact::Digest;
/// let _: Digest = Digest::Sha1([0; 20]);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Digest<S = Infallible> {
    /// SHA-1, available with `Digest<[u8; 20]>`.
    Sha1(S),
    /// SHA-256 (32 bytes).
    Sha256([u8; 32]),
}

/// Errors from parsing a digest string.
#[derive(Debug, Error)]
pub enum DigestParseError {
    /// Algorithm prefix not supported by this digest type.
    #[error("unsupported digest algorithm: '{0}'")]
    UnknownAlgorithm(String),
    /// Hex decoding failed.
    #[error("invalid hex in digest: {0}")]
    InvalidHex(#[from] hex::FromHexError),
    /// Hex string had wrong length for the algorithm.
    #[error("wrong digest length: expected {expected} hex chars, got {actual}")]
    WrongLength {
        /// Required length.
        expected: usize,
        /// Length actually seen.
        actual: usize,
    },
}

impl<S: Sha1> Digest<S> {
    /// Return the algorithm name.
    pub fn algo(&self) -> &'static str {
        match self {
            Self::Sha1(_) => "sha1",
            Self::Sha256(_) => "sha256",
        }
    }

    /// Return the lowercase hexadecimal digest.
    pub fn hex(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Return the raw bytes of the digest.
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Sha1(bytes) => bytes.as_bytes(),
            Self::Sha256(bytes) => bytes,
        }
    }
}

impl Digest {
    /// Return the fixed SHA-256 bytes; this digest cannot contain SHA-1.
    #[must_use]
    pub fn as_sha256_array(&self) -> [u8; 32] {
        match self {
            Self::Sha1(never) => match *never {},
            Self::Sha256(bytes) => *bytes,
        }
    }
}

pub(super) fn decode<const N: usize>(text: &str) -> Result<[u8; N], DigestParseError> {
    if text.len() != 2 * N {
        return Err(DigestParseError::WrongLength {
            expected: 2 * N,
            actual: text.len(),
        });
    }
    let mut bytes = [0; N];
    hex::decode_to_slice(text, &mut bytes)?;
    Ok(bytes)
}

impl<S: Sha1> FromStr for Digest<S> {
    type Err = DigestParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.split_once(':') {
            Some(("sha1", value)) => S::from_hex(value).map(Self::Sha1),
            Some(("sha256", value)) => decode(value).map(Self::Sha256),
            _ => Err(DigestParseError::UnknownAlgorithm(
                text.split(':').next().unwrap_or(text).into(),
            )),
        }
    }
}

impl<S: Sha1> fmt::Display for Digest<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algo(), self.hex())
    }
}

impl<S: Sha1> serde::Serialize for Digest<S> {
    fn serialize<T: serde::Serializer>(&self, serializer: T) -> Result<T::Ok, T::Error> {
        serializer.collect_str(self)
    }
}

impl<'de, S: Sha1> serde::Deserialize<'de> for Digest<S> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Known SHA-256 of "hello": 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
    const HELLO_HEX: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    #[test]
    fn from_str_round_trip() {
        let s = format!("sha256:{HELLO_HEX}");
        let d: Digest = s.parse().unwrap();
        assert_eq!(d.to_string(), s);
    }

    #[test]
    fn display_round_trips() {
        let d = format!("sha256:{HELLO_HEX}").parse::<Digest>().unwrap();
        let s = d.to_string();
        let d2: Digest = s.parse().unwrap();
        assert_eq!(d, d2);
    }

    #[test]
    fn algo_returns_sha256() {
        let d: Digest = Digest::Sha256([0x42; 32]);
        assert_eq!(d.algo(), "sha256");
    }

    #[test]
    fn hex_length_is_64() {
        let d: Digest = Digest::Sha256([0x42; 32]);
        assert_eq!(d.hex().len(), 64);
    }

    #[test]
    fn as_bytes_length_is_32() {
        let d: Digest = Digest::Sha256([0x42; 32]);
        assert_eq!(d.as_bytes().len(), 32);
    }

    #[test]
    fn err_unknown_algorithm() {
        let err = "md5:abc".parse::<Digest>().unwrap_err();
        assert!(matches!(err, DigestParseError::UnknownAlgorithm(_)));
    }

    #[test]
    fn err_wrong_length() {
        let err = "sha256:tooshort".parse::<Digest>().unwrap_err();
        match err {
            DigestParseError::WrongLength { expected, actual } => {
                assert_eq!(expected, 64);
                assert_eq!(actual, 8);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn err_invalid_hex() {
        // 64 chars but invalid hex (z is not valid hex)
        let bad_hex = "z".repeat(64);
        let err = format!("sha256:{bad_hex}").parse::<Digest>().unwrap_err();
        assert!(matches!(err, DigestParseError::InvalidHex(_)));
    }

    #[test]
    fn sha1_requires_enabled_storage() {
        let text = format!("sha1:{}", "AB".repeat(20));
        assert!(matches!(
            text.parse::<Digest>(),
            Err(DigestParseError::UnknownAlgorithm(_))
        ));
        let digest: Digest<[u8; 20]> = text.parse().unwrap();
        assert_eq!(digest, Digest::Sha1([0xab; 20]));
        assert_eq!(digest.to_string(), text.to_lowercase());
        let json = serde_json::to_string(&digest).unwrap();
        assert!(serde_json::from_str::<Digest>(&json).is_err());
        assert_eq!(
            serde_json::from_str::<Digest<[u8; 20]>>(&json).unwrap(),
            digest
        );
        let sha256 = format!("sha256:{}", "AB".repeat(32));
        assert_eq!(
            sha256.parse::<Digest<[u8; 20]>>().unwrap(),
            Digest::Sha256([0xab; 32])
        );
        for value in ["ab".repeat(19), "ab".repeat(21), "gg".repeat(20)] {
            assert!(format!("sha1:{value}").parse::<Digest<[u8; 20]>>().is_err());
        }
    }
}
