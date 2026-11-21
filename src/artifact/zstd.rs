// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::Media;
use std::marker::PhantomData;

/// The Zstd transport format for `T`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zstd<T>(PhantomData<T>);

impl<T: Media> Zstd<T> {
    const BYTES: [u8; 128] = {
        let raw = T::MEDIA_TYPE.as_bytes();
        let suffix = b"+zstd";
        let mut bytes = [0; 128];
        assert!(
            raw.len() <= bytes.len() - suffix.len(),
            "compressed media type exceeds capacity"
        );
        let (base, tail) = bytes.split_at_mut(raw.len());
        base.copy_from_slice(raw);
        tail.split_at_mut(suffix.len()).0.copy_from_slice(suffix);
        bytes
    };
}

impl<T: Media> Media for Zstd<T> {
    const MEDIA_TYPE: &'static str =
        match str::from_utf8(Self::BYTES.split_at(T::MEDIA_TYPE.len() + 5).0) {
            Ok(value) => value,
            Err(_) => panic!("invalid media type"),
        };
}
