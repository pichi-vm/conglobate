// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeMap};
use std::{collections::BTreeSet, fmt};

/// Named bidirectional console used by the build appliance.
pub const PORT_NAME: &str = "dev.pichi.build";
pub const PROTOCOL_VERSION: u32 = 1;
pub const BUILD_INPUT_OP: &str = "build-input";

/// The guest requests input after opening its console.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub v: u32,
    pub op: String,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            v: PROTOCOL_VERSION,
            op: BUILD_INPUT_OP.into(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error("build-port request unsupported version {0}")]
    Version(u32),

    #[error("build-port request unknown op {0:?}")]
    Operation(String),
}

impl Request {
    pub fn validate(&self) -> Result<(), RequestError> {
        if self.v != PROTOCOL_VERSION {
            return Err(RequestError::Version(self.v));
        }
        if self.op != BUILD_INPUT_OP {
            return Err(RequestError::Operation(self.op.clone()));
        }
        Ok(())
    }
}

/// Locators supplied by the host. Resolving paths and Git refs is builder work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Input {
    pub git_ref: String,
    pub plan: String,
    pub lock: String,
    pub context: Option<String>,
    pub location: Option<String>,
}

/// Exactly one response to the guest request; success and error fields cannot mix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    Success(Input),

    Failure(String),
}

impl Serialize for Response {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let count = match self {
            Self::Success(input) => {
                5 + usize::from(input.context.is_some()) + usize::from(input.location.is_some())
            }
            Self::Failure(_) => 3,
        };
        let mut map = serializer.serialize_map(Some(count))?;
        map.serialize_entry("v", &PROTOCOL_VERSION)?;
        map.serialize_entry("ok", &matches!(self, Self::Success(_)))?;
        match self {
            Self::Success(input) => {
                map.serialize_entry("ref", &input.git_ref)?;
                map.serialize_entry("plan", &input.plan)?;
                map.serialize_entry("lock", &input.lock)?;
                if let Some(context) = &input.context {
                    map.serialize_entry("context", context)?;
                }
                if let Some(location) = &input.location {
                    map.serialize_entry("location", location)?;
                }
            }
            Self::Failure(error) => map.serialize_entry("error", error)?,
        }
        map.end()
    }
}

struct ResponseVisitor;

impl ResponseVisitor {
    fn text<'de, A: de::MapAccess<'de>>(map: &mut A, name: &str) -> Result<String, A::Error> {
        let value = map
            .next_value::<String>()
            .map_err(|_| de::Error::custom(format!("invalid string field {name}")))?;
        if value.is_empty() || value.contains('\0') {
            return Err(de::Error::custom(format!(
                "field {name} is empty or contains NUL"
            )));
        }
        Ok(value)
    }
}

impl<'de> de::Visitor<'de> for ResponseVisitor {
    type Value = Response;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a build-input response map")
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Response, A::Error> {
        const FIELDS: &[&str] = &[
            "v", "ok", "ref", "plan", "lock", "context", "location", "error",
        ];
        let mut seen = BTreeSet::new();
        let (mut version, mut ok) = (None, None);
        let (mut git_ref, mut plan, mut lock, mut context, mut location, mut error) =
            (None, None, None, None, None, None);
        while let Some(name) = map.next_key::<String>()? {
            if !seen.insert(name.clone()) {
                return Err(de::Error::custom(format!("duplicate field {name}")));
            }
            match name.as_str() {
                "v" => version = Some(map.next_value::<u32>()?),
                "ok" => ok = Some(map.next_value::<bool>()?),
                "ref" => git_ref = Some(Self::text(&mut map, "ref")?),
                "plan" => plan = Some(Self::text(&mut map, "plan")?),
                "lock" => lock = Some(Self::text(&mut map, "lock")?),
                "context" => context = Some(Self::text(&mut map, "context")?),
                "location" => location = Some(Self::text(&mut map, "location")?),
                "error" => error = Some(Self::text(&mut map, "error")?),
                _ => return Err(de::Error::unknown_field(&name, FIELDS)),
            }
        }
        if version.ok_or_else(|| de::Error::missing_field("v"))? != PROTOCOL_VERSION {
            return Err(de::Error::custom("build-port protocol version must be 1"));
        }
        if !ok.ok_or_else(|| de::Error::missing_field("ok"))? {
            for name in ["ref", "plan", "lock", "context", "location"] {
                if seen.contains(name) {
                    return Err(de::Error::custom(format!(
                        "failure response contains {name}"
                    )));
                }
            }
            return Ok(Response::Failure(
                error.ok_or_else(|| de::Error::missing_field("error"))?,
            ));
        }
        if error.is_some() {
            return Err(de::Error::custom("success response contains error"));
        }
        Ok(Response::Success(Input {
            git_ref: git_ref.ok_or_else(|| de::Error::missing_field("ref"))?,
            plan: plan.ok_or_else(|| de::Error::missing_field("plan"))?,
            lock: lock.ok_or_else(|| de::Error::missing_field("lock"))?,
            context,
            location,
        }))
    }
}

impl<'de> Deserialize<'de> for Response {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ResponseVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_rejects_unknown_operations_versions_and_fields() {
        Request::default().validate().unwrap();
        assert!(
            Request {
                v: 2,
                ..Request::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            Request {
                op: "other".into(),
                ..Request::default()
            }
            .validate()
            .is_err()
        );
        for text in [
            r#"{"v":1,"op":"build-input","extra":true}"#,
            r#"{"v":1,"v":1,"op":"build-input"}"#,
        ] {
            assert!(serde_json::from_str::<Request>(text).is_err());
        }
    }

    #[test]
    fn response_preserves_flat_wire_shape() {
        let success = Response::Success(Input {
            git_ref: "refs/heads/main".into(),
            plan: "pichi.yaml".into(),
            lock: "pichi.lock".into(),
            context: None,
            location: None,
        });
        let expected =
            r#"{"v":1,"ok":true,"ref":"refs/heads/main","plan":"pichi.yaml","lock":"pichi.lock"}"#;
        assert_eq!(serde_json::to_string(&success).unwrap(), expected);
        assert_eq!(serde_json::from_str::<Response>(expected).unwrap(), success);
        let failure = Response::Failure("missing input".into());
        let expected = r#"{"v":1,"ok":false,"error":"missing input"}"#;
        assert_eq!(serde_json::to_string(&failure).unwrap(), expected);
        assert_eq!(serde_json::from_str::<Response>(expected).unwrap(), failure);
    }

    #[test]
    fn response_rejects_ambiguous_or_invalid_envelopes() {
        for text in [
            r#"{"v":1,"v":1,"ok":false,"error":"x"}"#,
            r#"{"v":2,"ok":false,"error":"x"}"#,
            r#"{"v":1,"ok":false,"error":"x","ref":"main"}"#,
            r#"{"v":1,"ok":true,"error":"x"}"#,
            r#"{"v":1,"ok":true,"ref":"main","plan":"p","lock":"l","context":null}"#,
            r#"{"v":1,"ok":true,"ref":"","plan":"p","lock":"l"}"#,
            r#"{"v":1,"ok":false,"error":"x","commit":"secret"}"#,
        ] {
            assert!(serde_json::from_str::<Response>(text).is_err(), "{text}");
        }
    }
}
