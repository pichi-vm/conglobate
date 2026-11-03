// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// A document version encoded as the integer `V`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Version<const V: usize>;

impl<const V: usize> Serialize for Version<V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        V.serialize(serializer)
    }
}

impl<'de, const V: usize> Deserialize<'de> for Version<V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if usize::deserialize(deserializer)? != V {
            return Err(de::Error::custom(format_args!("expected version {V}")));
        }
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::Version;

    #[test]
    fn json_and_cbor_encode_the_const_integer() {
        let version = Version::<256>;
        assert_eq!(serde_json::to_string(&version).unwrap(), "256");
        assert_eq!(
            serde_json::from_str::<Version<256>>("256").unwrap(),
            version
        );
        let mut bytes = Vec::new();
        ciborium::into_writer(&version, &mut bytes).unwrap();
        assert_eq!(bytes, [0x19, 0x01, 0x00]);
        assert_eq!(
            ciborium::from_reader::<Version<256>, _>(bytes.as_slice()).unwrap(),
            version
        );
        assert!(ciborium::from_reader::<Version<1>, _>(bytes.as_slice()).is_err());
    }

    #[test]
    fn deserialization_requires_the_expected_integer() {
        assert_eq!(serde_json::from_str::<Version<1>>("1").unwrap(), Version);
        for text in ["0", "2", "-1", "1.0", "\"1\"", "null", "[]", "{}"] {
            assert!(serde_json::from_str::<Version<1>>(text).is_err(), "{text}");
        }
    }
}
