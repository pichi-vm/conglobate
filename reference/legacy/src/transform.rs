// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Ordered, typed DTB edits. Parsing validates the entire sequence before execution.

use devtree::{OwnedNode, OwnedProperty, OwnedTree, Tree};
use regex::Regex;
use serde::Deserialize;

use crate::{Error, template::Template};

/// Default recipe bytes, included in the inventory by the build orchestrator.
pub const DEFAULT_YAML: &str = "- op: regex\n  if:\n    exists: /chosen/bootargs\n  property: /chosen/bootargs\n  match: '(^|\\s)corium=[^\\s]*'\n  replace: '${1}corium=${CARAPACE}'\n";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Condition {
    exists: String,
}

// Explicit variants avoid serde flatten's interaction with deny_unknown_fields.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
enum RawOperation {
    Set {
        property: String,
        #[serde(rename = "if")]
        condition: Option<Condition>,
        #[serde(rename = "type")]
        kind: ValueType,
        data: Option<serde_json::Value>,
    },
    Remove {
        property: String,
        #[serde(rename = "if")]
        condition: Option<Condition>,
    },
    Regex {
        property: String,
        #[serde(rename = "if")]
        condition: Option<Condition>,
        #[serde(rename = "match")]
        pattern: String,
        replace: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ValueType {
    String,
    Strings,
    U32,
    U64,
    Cells,
    Empty,
    Bytes,
}

#[derive(Debug)]
enum Value {
    Strings(Vec<Template>),
    Bytes(Vec<u8>),
}

impl Value {
    fn parse(kind: ValueType, data: Option<serde_json::Value>) -> Result<Self, Error> {
        if matches!(kind, ValueType::Empty) {
            if data.is_some() {
                return Err(Error::Invalid("empty values must omit data".into()));
            }
            return Ok(Self::Bytes(Vec::new()));
        }
        let data = data.ok_or_else(|| Error::Invalid("set requires data".into()))?;
        Ok(match kind {
            ValueType::String => Self::Strings(vec![Template::parse(
                &serde_json::from_value::<String>(data)?,
                None,
            )?]),
            ValueType::Strings => Self::Strings(
                serde_json::from_value::<Vec<String>>(data)?
                    .iter()
                    .map(|s| Template::parse(s, None))
                    .collect::<Result<_, _>>()?,
            ),
            ValueType::U32 => {
                Self::Bytes(serde_json::from_value::<u32>(data)?.to_be_bytes().to_vec())
            }
            ValueType::U64 => {
                Self::Bytes(serde_json::from_value::<u64>(data)?.to_be_bytes().to_vec())
            }
            ValueType::Cells => Self::Bytes(
                serde_json::from_value::<Vec<u32>>(data)?
                    .into_iter()
                    .flat_map(u32::to_be_bytes)
                    .collect(),
            ),
            ValueType::Bytes => {
                let text = serde_json::from_value::<String>(data)?;
                let hex: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
                if !hex.len().is_multiple_of(2) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(Error::Invalid(
                        "bytes data must contain whole hexadecimal bytes".into(),
                    ));
                }
                let bytes = (0..hex.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                    .collect();
                Self::Bytes(bytes)
            }
            ValueType::Empty => unreachable!(),
        })
    }

    fn bytes(&self, carapace: Option<&str>) -> Result<Vec<u8>, Error> {
        match self {
            Self::Bytes(bytes) => Ok(bytes.clone()),
            Self::Strings(templates) => {
                let mut bytes = Vec::new();
                for template in templates {
                    bytes.extend_from_slice(template.expand(carapace, None)?.as_bytes());
                    bytes.push(0);
                }
                Ok(bytes)
            }
        }
    }
}

#[derive(Debug)]
struct PropertyPath {
    node: String,
    name: String,
}

impl PropertyPath {
    fn parse(path: &str) -> Result<Self, Error> {
        let segments: Vec<_> = path
            .strip_prefix('/')
            .unwrap_or_default()
            .split('/')
            .collect();
        if !path.starts_with('/')
            || segments.len() > 64
            || segments.iter().any(|s| {
                s.is_empty()
                    || *s == "."
                    || *s == ".."
                    || !s.is_ascii()
                    || s.bytes()
                        .any(|b| b.is_ascii_control() || b.is_ascii_whitespace())
            })
        {
            return Err(Error::Invalid(format!(
                "invalid absolute property path {path:?}"
            )));
        }
        let (node, name) = path.rsplit_once('/').unwrap();
        Ok(Self {
            node: if node.is_empty() {
                "/".into()
            } else {
                node.into()
            },
            name: name.into(),
        })
    }

    fn get<'a>(&self, tree: &'a OwnedTree) -> Option<&'a OwnedProperty> {
        tree.find_path(&self.node)?.property(&self.name)
    }

    fn node_mut<'a>(
        &self,
        tree: &'a mut OwnedTree,
        create: bool,
    ) -> Result<&'a mut OwnedNode, Error> {
        let mut node = tree.root_mut();
        for segment in self.node.split('/').filter(|s| !s.is_empty()) {
            if create && node.child(segment).is_none() {
                node.set_child(OwnedNode::new(segment));
            }
            node = node.child_mut(segment).ok_or_else(|| self.missing())?;
        }
        Ok(node)
    }

    fn missing(&self) -> Error {
        Error::Invalid(format!(
            "missing property {}/{}",
            self.node.trim_end_matches('/'),
            self.name
        ))
    }
}

#[derive(Debug)]
enum Action {
    Set(Value),
    Remove,
    Regex { regex: Regex, template: Template },
}

#[derive(Debug)]
struct Operation {
    target: PropertyPath,
    condition: Option<PropertyPath>,
    action: Action,
}

impl TryFrom<RawOperation> for Operation {
    type Error = Error;

    fn try_from(raw: RawOperation) -> Result<Self, Error> {
        let (property, condition, action) = match raw {
            RawOperation::Set {
                property,
                condition,
                kind,
                data,
            } => (property, condition, Action::Set(Value::parse(kind, data)?)),
            RawOperation::Remove {
                property,
                condition,
            } => (property, condition, Action::Remove),
            RawOperation::Regex {
                property,
                condition,
                pattern,
                replace,
            } => {
                let regex = Regex::new(&pattern)
                    .map_err(|e| Error::Invalid(format!("invalid regex: {e}")))?;
                if regex.capture_names().flatten().any(|n| {
                    !n.starts_with(|c: char| c.is_ascii_lowercase())
                        || !n
                            .chars()
                            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                }) {
                    return Err(Error::Invalid("named captures must use lowercase names; uppercase names are reserved for build variables".into()));
                }
                let template = Template::parse(&replace, Some(&regex))?;
                (property, condition, Action::Regex { regex, template })
            }
        };
        Ok(Self {
            target: PropertyPath::parse(&property)?,
            condition: condition
                .map(|c| PropertyPath::parse(&c.exists))
                .transpose()?,
            action,
        })
    }
}

impl Operation {
    fn apply(&self, tree: &mut OwnedTree, carapace: Option<&str>) -> Result<(), Error> {
        if self
            .condition
            .as_ref()
            .is_some_and(|p| p.get(tree).is_none())
        {
            return Ok(());
        }
        match &self.action {
            Action::Set(value) => {
                let value =
                    OwnedProperty::new(&self.target.name).with_bytes(value.bytes(carapace)?);
                self.target.node_mut(tree, true)?.set_property(value);
            }
            Action::Remove => {
                self.target
                    .node_mut(tree, false)?
                    .remove_property(&self.target.name)
                    .ok_or_else(|| self.target.missing())?;
            }
            Action::Regex { regex, template } => {
                let prop = self.target.get(tree).ok_or_else(|| self.target.missing())?;
                // Check raw bytes rather than relying on a string accessor accepting stringlists.
                let bytes = prop.as_ref();
                let content = bytes
                    .strip_suffix(&[0])
                    .filter(|b| !b.contains(&0))
                    .and_then(|b| std::str::from_utf8(b).ok())
                    .ok_or_else(|| {
                        Error::Invalid(format!(
                            "{}/{} is not a single NUL-terminated UTF-8 string",
                            self.target.node, self.target.name
                        ))
                    })?;
                let mut output = String::new();
                let mut end = 0;
                for captures in regex.captures_iter(content) {
                    let matched = captures.get(0).unwrap();
                    output.push_str(&content[end..matched.start()]);
                    output.push_str(&template.expand(carapace, Some(&captures))?);
                    end = matched.end();
                }
                output.push_str(&content[end..]);
                self.target
                    .node_mut(tree, false)?
                    .set_property(OwnedProperty::new(&self.target.name).with_str(&output));
            }
        }
        Ok(())
    }
}

/// A validated authored sequence, or the implicit default sequence.
#[derive(Debug)]
pub struct Transform {
    implicit: bool,
    operations: Vec<Operation>,
}

impl Transform {
    /// `None` selects defaults. Any authored file, including empty, replaces them.
    pub fn parse(authored: Option<&str>) -> Result<Self, Error> {
        let text = authored.unwrap_or(DEFAULT_YAML);
        let raw: Vec<RawOperation> =
            crate::yaml::from_str::<Option<Vec<RawOperation>>>(text)?.unwrap_or_default();
        let operations = raw
            .into_iter()
            .map(Operation::try_from)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            implicit: authored.is_none(),
            operations,
        })
    }

    /// Produce a new DTB; callers retain their input on any error.
    pub fn apply(
        &self,
        dtb: Option<&[u8]>,
        carapace: Option<&str>,
    ) -> Result<Option<Vec<u8>>, Error> {
        let Some(dtb) = dtb else {
            return if self.implicit {
                Ok(None)
            } else {
                Err(Error::Invalid("authored transform requires a DTB".into()))
            };
        };
        let tree: Tree<'_> = Tree::parse(dtb).map_err(|e| Error::Dtb(e.to_string()))?;
        let mut owned = OwnedTree::from(&tree);
        let original = owned.clone();
        for (index, op) in self.operations.iter().enumerate() {
            op.apply(&mut owned, carapace)
                .map_err(|e| Error::Invalid(format!("transform operation {}: {e}", index + 1)))?;
        }
        if owned == original {
            return Ok(Some(dtb.to_vec()));
        }
        let mut output = owned.encode().map_err(|e| Error::Dtb(e.to_string()))?;
        // devtree's owned encoder currently emits boot_cpuid_phys=0. Preserve
        // the FDT header's compatibility version and boot CPU ID (offsets 24..32)
        // after its parser has validated the input header. Never change either
        // while editing properties. Lengths/offsets are recomputed by the library.
        output[24..32].copy_from_slice(&dtb[24..32]);
        let _: Tree<'_> = Tree::parse(&output).map_err(|e| Error::Dtb(e.to_string()))?;
        Ok(Some(output))
    }
}
