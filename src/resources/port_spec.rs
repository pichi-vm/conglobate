// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};
use std::fmt;

/// One ingress port-list entry: a single port, an inclusive `[low, high]`
/// range, or `*` (all ports).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortSpec {
    /// A single port.
    Port(u16),
    /// An inclusive `[low, high]` range.
    Range(u16, u16),
    /// All ports (`*`).
    All,
}

impl Serialize for PortSpec {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            PortSpec::Port(p) => s.serialize_u16(*p),
            PortSpec::Range(lo, hi) => {
                use serde::ser::SerializeSeq;
                let mut seq = s.serialize_seq(Some(2))?;
                seq.serialize_element(lo)?;
                seq.serialize_element(hi)?;
                seq.end()
            }
            PortSpec::All => s.serialize_str("*"),
        }
    }
}

impl<'de> Deserialize<'de> for PortSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = PortSpec;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a port (integer), an inclusive [low, high] range, or \"*\"")
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<PortSpec, E> {
                u16::try_from(v)
                    .map(PortSpec::Port)
                    .map_err(|_| E::custom(format!("port {v} out of range")))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<PortSpec, E> {
                if v == "*" {
                    Ok(PortSpec::All)
                } else {
                    Err(E::custom(format!(
                        "expected a port, range, or \"*\", got {v:?}"
                    )))
                }
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<PortSpec, A::Error> {
                use serde::de::Error as _;
                let lo: u16 = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::custom("range needs [low, high]"))?;
                let hi: u16 = seq
                    .next_element()?
                    .ok_or_else(|| A::Error::custom("range needs [low, high]"))?;
                if seq.next_element::<u16>()?.is_some() {
                    return Err(A::Error::custom("range has more than two elements"));
                }
                Ok(PortSpec::Range(lo, hi))
            }
        }
        d.deserialize_any(V)
    }
}
