// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::digest::{DigestParseError, decode};
use std::convert::Infallible;

/// Storage for the optional SHA-1 variant of a digest.
pub trait Sha1: Sized {
    /// Decode the fixed-width SHA-1 bytes, or reject this algorithm.
    fn from_hex(text: &str) -> Result<Self, DigestParseError>;
    /// Borrow the raw digest bytes.
    fn as_bytes(&self) -> &[u8];
}

impl Sha1 for Infallible {
    fn from_hex(_: &str) -> Result<Self, DigestParseError> {
        Err(DigestParseError::UnknownAlgorithm("sha1".into()))
    }

    fn as_bytes(&self) -> &[u8] {
        match *self {}
    }
}

impl Sha1 for [u8; 20] {
    fn from_hex(text: &str) -> Result<Self, DigestParseError> {
        decode(text)
    }

    fn as_bytes(&self) -> &[u8] {
        self
    }
}
