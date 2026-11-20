// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::{fmt, ops::Deref, str::FromStr};

/// A value encoded as its canonical `Display` string.
///
/// Parsing and value constraints belong to `T`. Deserialization additionally
/// requires that displaying the parsed value reproduces the input exactly.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AsString<T>(pub T);

impl<T> From<T> for AsString<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for AsString<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: fmt::Display> Serialize for AsString<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl<'de, T> Deserialize<'de> for AsString<T>
where
    T: FromStr + fmt::Display,
    T::Err: fmt::Display,
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        let value: T = text.parse().map_err(serde::de::Error::custom)?;
        if value.to_string() != text {
            return Err(serde::de::Error::custom(
                "expected a canonical string representation",
            ));
        }
        Ok(Self(value))
    }
}

#[cfg(test)]
mod tests {
    use super::AsString;
    use std::num::NonZeroU64;

    #[test]
    fn value_constraints_belong_to_the_wrapped_type() {
        assert_eq!(
            serde_json::from_str::<AsString<u32>>(r#""0""#).unwrap(),
            AsString(0)
        );
        assert!(serde_json::from_str::<AsString<NonZeroU64>>(r#""0""#).is_err());
        let value = AsString(NonZeroU64::new(u64::MAX).unwrap());
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(encoded, format!("\"{}\"", u64::MAX));
        assert_eq!(
            serde_json::from_str::<AsString<NonZeroU64>>(&encoded).unwrap(),
            value
        );
    }

    #[test]
    fn only_canonical_strings_are_accepted() {
        for invalid in [
            r#""01""#,
            r#""+1""#,
            r#""-1""#,
            r#""4294967296""#,
            "1",
            "null",
        ] {
            assert!(
                serde_json::from_str::<AsString<u32>>(invalid).is_err(),
                "{invalid}"
            );
        }
    }
}
