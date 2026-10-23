// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Validated transform descriptions; execution is implemented separately.
use super::{ImagePath, Presence, RecipeError};
use crate::template::Template;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConditionDocument {
    exists: String,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub(super) enum TransformDocument {
    Set {
        property: String,
        #[serde(rename = "if", default)]
        condition: Presence<ConditionDocument>,
        #[serde(rename = "type")]
        kind: String,
        #[serde(default)]
        data: Presence<serde_json::Value>,
    },
    Remove {
        property: String,
        #[serde(rename = "if", default)]
        condition: Presence<ConditionDocument>,
    },
    Regex {
        property: String,
        #[serde(rename = "if", default)]
        condition: Presence<ConditionDocument>,
        #[serde(rename = "match")]
        pattern: String,
        replace: String,
    },
}
#[derive(Debug)]
pub enum TransformSelection {
    Builtin,
    Authored(Vec<TransformOperation>),
}
#[derive(Debug)]
pub enum PropertyValue {
    String(Template),
    Strings(Vec<Template>),
    U32(u32),
    U64(u64),
    Cells(Vec<u32>),
    Empty,
    Bytes(Vec<u8>),
}
#[derive(Debug)]
pub enum TransformAction {
    Set(PropertyValue),
    Remove,
    Regex {
        pattern: regex::Regex,
        replacement: Template,
    },
}
#[derive(Debug)]
pub struct TransformOperation {
    property: String,
    exists: Option<String>,
    action: TransformAction,
}
impl TransformOperation {
    pub fn property(&self) -> &str {
        &self.property
    }
    pub fn exists(&self) -> Option<&str> {
        self.exists.as_deref()
    }
    pub fn action(&self) -> &TransformAction {
        &self.action
    }
    fn property_path(path: &str) -> Result<(), RecipeError> {
        let _ = ImagePath::try_from(path.to_owned())?;
        if path == "/"
            || path
                .split('/')
                .skip(1)
                .any(|p| p.is_empty() || p == "." || p == "..")
        {
            return Err(RecipeError::Invalid("invalid DTB property path"));
        }
        Ok(())
    }
}
impl TryFrom<TransformDocument> for TransformOperation {
    type Error = RecipeError;
    fn try_from(raw: TransformDocument) -> Result<Self, Self::Error> {
        let (property, condition, action) = match raw {
            TransformDocument::Remove {
                property,
                condition,
            } => (property, condition, TransformAction::Remove),
            TransformDocument::Regex {
                property,
                condition,
                pattern,
                replace,
            } => (
                property,
                condition,
                TransformAction::Regex {
                    pattern: regex::Regex::new(&pattern)?,
                    replacement: Template::replacement(&replace)?,
                },
            ),
            TransformDocument::Set {
                property,
                condition,
                kind,
                data,
            } => (
                property,
                condition,
                TransformAction::Set(PropertyValue::decode(&kind, data)?),
            ),
        };
        Self::property_path(&property)?;
        let exists = condition.into_option().map(|c| c.exists);
        if let Some(path) = &exists {
            Self::property_path(path)?;
        }
        Ok(Self {
            property,
            exists,
            action,
        })
    }
}
impl PropertyValue {
    fn decode(kind: &str, data: Presence<serde_json::Value>) -> Result<Self, RecipeError> {
        use serde_json::Value;
        let invalid = RecipeError::Invalid("DTB set type/data mismatch");
        match (kind, data) {
            ("empty", Presence::Absent) => Ok(Self::Empty),
            ("string", Presence::Present(Value::String(s))) if !s.contains('\0') => {
                Ok(Self::String(Template::set(&s)?))
            }
            ("strings", Presence::Present(Value::Array(a))) => Ok(Self::Strings(
                a.into_iter()
                    .map(|v| match v {
                        Value::String(s) if !s.contains('\0') => Ok(Template::set(&s)?),
                        _ => Err(RecipeError::Invalid("expected NUL-free string")),
                    })
                    .collect::<Result<_, _>>()?,
            )),
            ("u32", Presence::Present(Value::Number(n))) => n
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .map(Self::U32)
                .ok_or(invalid),
            ("u64", Presence::Present(Value::Number(n))) => {
                n.as_u64().map(Self::U64).ok_or(invalid)
            }
            ("cells", Presence::Present(Value::Array(a))) => Ok(Self::Cells(
                a.into_iter()
                    .map(|v| {
                        v.as_u64()
                            .and_then(|n| u32::try_from(n).ok())
                            .ok_or(RecipeError::Invalid("expected unsigned 32-bit cell"))
                    })
                    .collect::<Result<_, _>>()?,
            )),
            ("bytes", Presence::Present(Value::String(s))) => {
                let compact: String = s.chars().filter(|c| !c.is_ascii_whitespace()).collect();
                let bytes = hex::decode(compact)
                    .map_err(|_| RecipeError::Invalid("expected hexadecimal bytes"))?;
                Ok(Self::Bytes(bytes))
            }
            _ => Err(invalid),
        }
    }
}
