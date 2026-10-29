// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Messages on the build console.
//!
//! The build console is a named virtio console port that connects a
//! Conglobate appliance VM (the guest) to the program that started it, such
//! as Pichi (the host). The guest opens the port, sends one
//! [`BuildRequest`], and reads one [`BuildReply`] that says what to build.
//!
//! These are plain serde types. Both sides encode them as CBOR (a compact
//! binary form of JSON-like data) with a CBOR crate of their choice; this
//! library does not depend on one. A message is a single map with text
//! keys, so the encoding does not depend on the crate.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de, ser::SerializeMap};

/// Name of the virtio console port that carries the build messages.
pub const CONSOLE_NAME: &str = "dev.pichi.build";

/// Version of the build console messages described here. Every message
/// carries it as `v`.
pub const PROTOCOL_VERSION: u32 = 1;

/// Wire name of [`BuildOp::BuildInput`].
pub const BUILD_INPUT_OP: &str = "build-input";

/// What the guest asks the host for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildOp {
    /// Ask which commit, plan and lock to build.
    #[serde(rename = "build-input")]
    BuildInput,
}

/// The guest's request: `{"v": 1, "op": "build-input"}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildRequest {
    /// Message version; [`PROTOCOL_VERSION`] for requests made here.
    #[serde(rename = "v")]
    pub version: u32,
    /// What is asked for.
    pub op: BuildOp,
}

impl BuildRequest {
    /// The current version's build-input request.
    pub const fn build_input() -> Self {
        Self {
            version: PROTOCOL_VERSION,
            op: BuildOp::BuildInput,
        }
    }
}

impl Default for BuildRequest {
    fn default() -> Self {
        Self::build_input()
    }
}

/// What to build, sent by the host when it accepts the request.
///
/// Paths are relative to the repository root, as text. The guest checks
/// that they are non-empty and stay inside the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildInput {
    /// Git commit id or reference name to build (`ref` on the wire).
    pub git_ref: String,
    /// Path of the plan file.
    pub plan: String,
    /// Path of the lock file.
    pub lock: String,
    /// Build context directory; the repository root when absent.
    pub context: Option<String>,
    /// Where the repository came from, recorded in the output. It must not
    /// contain credentials.
    pub location: Option<String>,
}

/// The host's reply to a [`BuildRequest`].
///
/// On the wire this is one map with `v` and `ok`. When `ok` is true the map
/// also holds the [`BuildInput`] fields `ref`, `plan`, `lock`, and
/// optionally `context` and `location`. When `ok` is false it holds only
/// `error`, a message for the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildReply {
    /// The host accepted the request.
    Success(BuildInput),
    /// The host refused the request.
    Failure {
        /// Why the host refused.
        error: String,
    },
}

impl Serialize for BuildReply {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("v", &PROTOCOL_VERSION)?;
        match self {
            Self::Success(input) => {
                map.serialize_entry("ok", &true)?;
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
            Self::Failure { error } => {
                map.serialize_entry("ok", &false)?;
                map.serialize_entry("error", error)?;
            }
        }
        map.end()
    }
}

/// Every field a reply may have; [`BuildReply`] checks how they combine.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReplyFields {
    v: u32,
    ok: bool,
    error: Option<String>,
    #[serde(rename = "ref")]
    git_ref: Option<String>,
    plan: Option<String>,
    lock: Option<String>,
    context: Option<String>,
    location: Option<String>,
}

impl<'de> Deserialize<'de> for BuildReply {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let fields = ReplyFields::deserialize(deserializer)?;
        if fields.v != PROTOCOL_VERSION {
            return Err(de::Error::custom(format_args!(
                "build reply version must be {PROTOCOL_VERSION}"
            )));
        }
        if !fields.ok {
            let present = [
                ("ref", fields.git_ref.is_some()),
                ("plan", fields.plan.is_some()),
                ("lock", fields.lock.is_some()),
                ("context", fields.context.is_some()),
                ("location", fields.location.is_some()),
            ];
            if let Some((name, _)) = present.iter().find(|(_, present)| *present) {
                return Err(de::Error::custom(format_args!(
                    "failed build reply must not have field {name}"
                )));
            }
            let error = fields
                .error
                .ok_or_else(|| de::Error::missing_field("error"))?;
            return Ok(Self::Failure { error });
        }
        if fields.error.is_some() {
            return Err(de::Error::custom(
                "successful build reply must not have field error",
            ));
        }
        Ok(Self::Success(BuildInput {
            git_ref: fields
                .git_ref
                .ok_or_else(|| de::Error::missing_field("ref"))?,
            plan: fields
                .plan
                .ok_or_else(|| de::Error::missing_field("plan"))?,
            lock: fields
                .lock
                .ok_or_else(|| de::Error::missing_field("lock"))?,
            context: fields.context,
            location: fields.location,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn success() -> BuildReply {
        BuildReply::Success(BuildInput {
            git_ref: "refs/heads/main".into(),
            plan: "plan.yaml".into(),
            lock: "images.lock".into(),
            context: None,
            location: None,
        })
    }

    #[test]
    fn request_wire_shape() {
        let json = serde_json::to_value(BuildRequest::build_input()).unwrap();
        assert_eq!(json, serde_json::json!({"v": 1, "op": "build-input"}));
        let parsed: BuildRequest = serde_json::from_value(json).unwrap();
        assert_eq!(parsed, BuildRequest::default());
        assert!(
            serde_json::from_value::<BuildRequest>(serde_json::json!({"v": 1, "op": "other"}))
                .is_err()
        );
        assert!(
            serde_json::from_value::<BuildRequest>(
                serde_json::json!({"v": 1, "op": "build-input", "extra": 1})
            )
            .is_err()
        );
        assert_eq!(
            serde_json::to_value(BuildOp::BuildInput).unwrap(),
            BUILD_INPUT_OP
        );
    }

    #[test]
    fn success_round_trips_in_wire_order() {
        let text = serde_json::to_string(&success()).unwrap();
        assert_eq!(
            text,
            r#"{"v":1,"ok":true,"ref":"refs/heads/main","plan":"plan.yaml","lock":"images.lock"}"#
        );
        assert_eq!(
            serde_json::from_str::<BuildReply>(&text).unwrap(),
            success()
        );
        let mut full = success();
        if let BuildReply::Success(input) = &mut full {
            input.context = Some("project".into());
            input.location = Some("https://example.invalid/repo".into());
        }
        let text = serde_json::to_string(&full).unwrap();
        assert_eq!(serde_json::from_str::<BuildReply>(&text).unwrap(), full);
    }

    #[test]
    fn failure_round_trips() {
        let failure = BuildReply::Failure {
            error: "no plan".into(),
        };
        let json = serde_json::to_value(&failure).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"v": 1, "ok": false, "error": "no plan"})
        );
        assert_eq!(serde_json::from_value::<BuildReply>(json).unwrap(), failure);
    }

    #[test]
    fn rejects_mixed_unknown_or_missing_fields() {
        for (json, needle) in [
            (
                serde_json::json!({"v": 1, "ok": false, "error": "x", "ref": "main"}),
                "ref",
            ),
            (
                serde_json::json!({"v": 1, "ok": true, "error": "x", "ref": "a", "plan": "p", "lock": "l"}),
                "error",
            ),
            (
                serde_json::json!({"v": 1, "ok": true, "plan": "p", "lock": "l"}),
                "ref",
            ),
            (serde_json::json!({"v": 1, "ok": false}), "error"),
            (
                serde_json::json!({"v": 2, "ok": false, "error": "x"}),
                "version",
            ),
            (serde_json::json!({"v": 1, "error": "x"}), "ok"),
            (
                serde_json::json!({"v": 1, "ok": false, "error": "x", "surprise": 1}),
                "surprise",
            ),
        ] {
            let error = serde_json::from_value::<BuildReply>(json)
                .unwrap_err()
                .to_string();
            assert!(error.contains(needle), "{needle}: {error}");
        }
    }
}
