// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Version-two inventory with strict schema and core deterministic CBOR.
//! Decoding validates representation, not authenticity or successful execution.
mod record;
mod schema;
use ciborium::Value;
use schema::*;
use sha2::{Digest as _, Sha256};

#[derive(Debug, thiserror::Error)]
#[error("inventory: {0}")]
pub struct InventoryError(String);
impl InventoryError {
    fn invalid(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}
#[derive(Debug)]
pub struct Inventory {
    document: Document,
    bytes: Vec<u8>,
}
impl Inventory {
    /// Decode the canonical versioned wire schema and validate its structure.
    /// This proves neither authenticity nor that the described build occurred.
    ///
    /// # Errors
    /// Rejects malformed/trailing CBOR, noncanonical encoding, unsupported or
    /// invalid schema values, inconsistent components and invalid ordering.
    pub fn decode(bytes: &[u8]) -> Result<Self, InventoryError> {
        let mut input = bytes;
        let value: Value = ciborium::from_reader(&mut input).map_err(InventoryError::invalid)?;
        if !input.is_empty() {
            return Err(InventoryError::invalid("trailing data"));
        }
        let value = CanonicalValue::new(value)?;
        if value.encode()? != bytes {
            return Err(InventoryError::invalid("noncanonical CBOR"));
        }
        let document: Document = value.0.deserialized().map_err(InventoryError::invalid)?;
        let inventory = Self::from_document(document)?;
        // Also rejects optional nulls, alternate enum representations and any
        // value a serde adapter accepted but the schema would not emit.
        if inventory.bytes != bytes {
            return Err(InventoryError::invalid(
                "noncanonical schema representation",
            ));
        }
        Ok(inventory)
    }
    fn from_document(document: Document) -> Result<Self, InventoryError> {
        document.validate()?;
        let value = Value::serialized(&document).map_err(InventoryError::invalid)?;
        let bytes = CanonicalValue::new(value)?.encode()?;
        Ok(Self { document, bytes })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn sha256(&self) -> [u8; 32] {
        Sha256::digest(&self.bytes).into()
    }
    pub fn version(&self) -> u32 {
        self.document.version
    }
}

/// Owned normalization; no mutable wire-value API escapes the encoder.
#[derive(Debug)]
struct CanonicalValue(Value);
impl CanonicalValue {
    fn new(value: Value) -> Result<Self, InventoryError> {
        let value = match value {
            Value::Map(entries) => {
                let mut sorted = Vec::with_capacity(entries.len());
                for (key, value) in entries {
                    let key = Self::new(key)?;
                    let value = Self::new(value)?;
                    sorted.push((key.encode()?, key.0, value.0));
                }
                sorted.sort_by(|a, b| a.0.cmp(&b.0));
                if sorted.windows(2).any(|w| w[0].0 == w[1].0) {
                    return Err(InventoryError::invalid("duplicate map key"));
                }
                Value::Map(
                    sorted
                        .into_iter()
                        .map(|(_, key, value)| (key, value))
                        .collect(),
                )
            }
            Value::Array(values) => Value::Array(
                values
                    .into_iter()
                    .map(Self::new)
                    .map(|v| v.map(|v| v.0))
                    .collect::<Result<_, _>>()?,
            ),
            Value::Float(_) | Value::Tag(_, _) => {
                return Err(InventoryError::invalid("floats and tags are forbidden"));
            }
            value => value,
        };
        Ok(Self(value))
    }
    fn encode(&self) -> Result<Vec<u8>, InventoryError> {
        let mut bytes = Vec::new();
        ciborium::into_writer(&self.0, &mut bytes).map_err(InventoryError::invalid)?;
        Ok(bytes)
    }
}
impl Document {
    fn validate(&self) -> Result<(), InventoryError> {
        if self.version != 2 || self.builder.name != "conglobate" || self.builder.version.is_empty()
        {
            return Err(InventoryError::invalid("unsupported version or builder"));
        }
        let size = match self.context.object_format {
            ObjectFormat::Sha1 => 20,
            ObjectFormat::Sha256 => 32,
        };
        if self.context.commit.len() != size || self.context.tree.len() != size {
            return Err(InventoryError::invalid("Git identity length mismatch"));
        }
        crate::path::ContextDirectory::try_from(std::path::Path::new(&self.context.subdirectory))
            .map_err(InventoryError::invalid)?;
        for selected in [&self.execution.recipe, &self.execution.lock] {
            crate::path::ContextPath::try_from(std::path::Path::new(&selected.path))
                .map_err(InventoryError::invalid)?;
        }
        if self.context.repository.is_empty() || self.context.repository.contains('\0') {
            return Err(InventoryError::invalid("invalid repository locator"));
        }
        if self
            .inputs
            .windows(2)
            .any(|w| w[0].reference >= w[1].reference)
        {
            return Err(InventoryError::invalid("inputs must be sorted and unique"));
        }
        for input in &self.inputs {
            input
                .reference
                .parse::<crate::image::ImageReference>()
                .map_err(InventoryError::invalid)?;
            if input.roles.is_empty()
                || input.roles.windows(2).any(|w| w[0] >= w[1])
                || input.carapace_root.as_ref().is_some_and(|r| r.len() != 32)
                || (input.carapace_root.is_none()
                    && input.roles.iter().any(|r| *r != Role::CarapaceBase))
            {
                return Err(InventoryError::invalid("invalid input roles/root"));
            }
        }
        let transform = &self.execution.transform;
        if (transform.source == TransformSource::Authored && transform.default_skipped)
            || (!transform.target_present && !transform.default_skipped)
        {
            return Err(InventoryError::invalid("invalid transform disposition"));
        }
        if self
            .execution
            .network_steps
            .windows(2)
            .any(|w| (&w[0].stage, w[0].step) >= (&w[1].stage, w[1].step))
        {
            return Err(InventoryError::invalid(
                "network steps must be in pipeline order and unique",
            ));
        }
        for formatter in &self.execution.formatters {
            formatter.validate()?;
        }
        if self.outputs.scutes.is_empty() != self.outputs.carapace_root.is_none()
            || self
                .outputs
                .carapace_root
                .as_ref()
                .is_some_and(|r| r.len() != 32)
            || self.outputs.pmi.is_some() != self.outputs.dtb.is_some()
            || (self.outputs.scutes.is_empty() && self.outputs.pmi.is_none())
        {
            return Err(InventoryError::invalid(
                "invalid output component combination",
            ));
        }
        Ok(())
    }
}
impl Formatter {
    fn validate(&self) -> Result<(), InventoryError> {
        if self.stage != Stage::Bootstrap
            || self.profile.is_empty()
            || self.runtime.is_empty()
            || self.runtime.windows(2).any(|w| w[0].path >= w[1].path)
        {
            return Err(InventoryError::invalid(
                "invalid formatter profile/runtime ordering",
            ));
        }
        for file in self.runtime.iter().chain(std::iter::once(&self.executable)) {
            crate::path::ContextPath::try_from(std::path::Path::new(&file.path))
                .map_err(InventoryError::invalid)?;
        }
        if !self.runtime.iter().any(|r| {
            r.path == self.executable.path && r.digest.digest == self.executable.digest.digest
        }) || self.arguments.iter().any(|arg| arg.contains('\0'))
        {
            return Err(InventoryError::invalid(
                "formatter executable or arguments mismatch",
            ));
        }
        let filesystem = &self.filesystem;
        crate::value::FilesystemSize::try_from(filesystem.size_bytes)
            .map_err(InventoryError::invalid)?;
        crate::value::FilesystemLabel::try_from(filesystem.label.clone())
            .map_err(InventoryError::invalid)?;
        if filesystem.logical_size_bytes != crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES {
            return Err(InventoryError::invalid(
                "invalid formatter logical geometry",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Debug)]
    struct Fixture(Document);
    impl Fixture {
        fn digest() -> Digest {
            Digest {
                algorithm: Algorithm::Sha256,
                digest: [7; 32],
            }
        }
        fn new() -> Self {
            Self(Document {
                version: 2,
                builder: Builder {
                    name: "conglobate".into(),
                    version: "test".into(),
                    executable: Self::digest(),
                },
                context: Context {
                    repository: "/local/repository".into(),
                    object_format: ObjectFormat::Sha1,
                    commit: vec![1; 20].into(),
                    subdirectory: ".".into(),
                    tree: vec![2; 20].into(),
                },
                inputs: vec![Input {
                    roles: vec![Role::CarapaceBase],
                    reference: "example.test/base:1".into(),
                    index: Self::digest(),
                    manifest: Self::digest(),
                    carapace_root: None,
                }],
                execution: Execution {
                    architecture: Architecture::Amd64,
                    recipe: SelectedDocument {
                        path: "recipes/image.yml".into(),
                        content: Self::digest(),
                    },
                    lock: SelectedDocument {
                        path: "refs.lock".into(),
                        content: Self::digest(),
                    },
                    transform: Transform {
                        source: TransformSource::Authored,
                        digest: Self::digest(),
                        target_present: true,
                        default_skipped: false,
                    },
                    formatters: vec![],
                    network_steps: vec![],
                },
                outputs: Outputs {
                    manifest: Self::digest(),
                    carapace_root: None,
                    scutes: vec![],
                    pmi: Some(Self::digest()),
                    dtb: Some(Self::digest()),
                },
            })
        }
        fn wire(&self) -> Wire {
            Wire(Value::serialized(&self.0).unwrap())
        }
        fn formatter() -> Formatter {
            Formatter {
                stage: Stage::Bootstrap,
                profile: "conglobate-ext4-v1".into(),
                profile_digest: Self::digest(),
                executable: RuntimeFile {
                    path: "usr/bin/mke2fs".into(),
                    digest: Self::digest(),
                },
                runtime: vec![
                    RuntimeFile {
                        path: "etc/mke2fs.conf".into(),
                        digest: Self::digest(),
                    },
                    RuntimeFile {
                        path: "usr/bin/mke2fs".into(),
                        digest: Self::digest(),
                    },
                ],
                configuration: Self::digest(),
                arguments: vec!["-L".into(), "rootfs".into(), "/dev/output".into()],
                filesystem: Filesystem {
                    kind: FilesystemType::Ext4,
                    size_bytes: 16 * 1024 * 1024,
                    logical_size_bytes: crate::value::FilesystemSize::LOGICAL_DEVICE_BYTES,
                    label: "rootfs".into(),
                },
            }
        }
    }
    #[derive(Debug)]
    struct Wire(Value);
    impl Wire {
        fn at(&mut self, path: &[&str]) -> &mut Value {
            let mut value = &mut self.0;
            for name in path {
                value = value
                    .as_map_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|(key, _)| key.as_text() == Some(name))
                    .map(|(_, value)| value)
                    .unwrap();
            }
            value
        }
        fn bytes(self) -> Vec<u8> {
            CanonicalValue::new(self.0).unwrap().encode().unwrap()
        }
        fn raw(&self) -> Vec<u8> {
            let mut bytes = vec![];
            ciborium::into_writer(&self.0, &mut bytes).unwrap();
            bytes
        }
    }
    #[test]
    fn version_two_roundtrip_including_formatter_and_network_receipts() {
        let mut fixture = Fixture::new();
        fixture.0.execution.formatters.push(Fixture::formatter());
        fixture.0.execution.network_steps = vec![
            NetworkStep {
                stage: Stage::Bootstrap,
                step: 4,
            },
            NetworkStep {
                stage: Stage::Firmware,
                step: 0,
            },
        ];
        let inventory = Inventory::from_document(fixture.0).unwrap();
        let decoded = Inventory::decode(inventory.bytes()).unwrap();
        assert_eq!(decoded.version(), 2);
        assert_eq!(decoded.bytes(), inventory.bytes());
        assert_eq!(decoded.sha256(), inventory.sha256());
        let value: Value = ciborium::from_reader(decoded.bytes()).unwrap();
        let mut wire = Wire(value);
        assert_eq!(
            wire.at(&["builder", "executable", "digest"]),
            &Value::Bytes(vec![7; 32])
        );
        assert!(
            wire.at(&["execution"])
                .as_map()
                .unwrap()
                .iter()
                .all(|(k, _)| k.as_text() != Some("resolv_conf"))
        );
    }
    #[test]
    fn core_deterministic_key_order_uses_encoded_bytes() {
        let value = Value::Map(vec![
            (Value::Text("aa".into()), 0.into()),
            (Value::Text("b".into()), 0.into()),
        ]);
        assert_eq!(
            CanonicalValue::new(value).unwrap().encode().unwrap(),
            b"\xa2\x61b\x00\x62aa\x00"
        );
    }
    #[test]
    fn rejects_unknown_duplicate_noncanonical_and_trailing_encodings() {
        let bytes = Fixture::new().wire().bytes();
        assert_eq!(bytes[0], 0xa6);
        let mut nonminimal = vec![0xb8, 6];
        nonminimal.extend_from_slice(&bytes[1..]);
        let mut indefinite = vec![0xbf];
        indefinite.extend_from_slice(&bytes[1..]);
        indefinite.push(0xff);
        let mut trailing = bytes.clone();
        trailing.push(0);
        for invalid in [nonminimal, indefinite, trailing] {
            assert!(Inventory::decode(&invalid).is_err());
        }
        let mut duplicate = Fixture::new().wire();
        let entries = duplicate.0.as_map_mut().unwrap();
        entries.push(entries[0].clone());
        assert!(Inventory::decode(&duplicate.raw()).is_err());
        for path in [vec![], vec!["execution"], vec!["builder", "executable"]] {
            let mut wire = Fixture::new().wire();
            wire.at(&path)
                .as_map_mut()
                .unwrap()
                .push((Value::Text("unknown".into()), true.into()));
            assert!(Inventory::decode(&wire.bytes()).is_err());
        }
        let mut reversed: Value = ciborium::from_reader(bytes.as_slice()).unwrap();
        reversed.as_map_mut().unwrap().reverse();
        assert!(Inventory::decode(&Wire(reversed).raw()).is_err());
    }
    #[test]
    fn rejects_wrong_types_versions_roots_and_transform_dispositions() {
        for (path, value) in [
            (vec!["version"], Value::Integer(1.into())),
            (
                vec!["builder", "executable", "digest"],
                Value::Bytes(vec![0; 31]),
            ),
            (
                vec!["builder", "executable", "algorithm"],
                Value::Text("sha1".into()),
            ),
            (vec!["context", "commit"], Value::Bytes(vec![0; 32])),
            (
                vec!["execution", "recipe", "path"],
                Value::Text("../outside".into()),
            ),
            (
                vec!["execution", "transform", "default_skipped"],
                Value::Bool(true),
            ),
        ] {
            let mut wire = Fixture::new().wire();
            *wire.at(&path) = value;
            assert!(Inventory::decode(&wire.bytes()).is_err(), "{path:?}");
        }
        for value in [Value::Float(2.0), Value::Tag(0, Box::new(2.into()))] {
            let mut wire = Fixture::new().wire();
            *wire.at(&["version"]) = value;
            assert!(Inventory::decode(&wire.raw()).is_err());
        }
        let mut fixture = Fixture::new();
        fixture.0.inputs[0].roles = vec![Role::FirmwareTools];
        assert!(Inventory::from_document(fixture.0).is_err());
        let mut fixture = Fixture::new();
        fixture.0.outputs.pmi = None;
        assert!(Inventory::from_document(fixture.0).is_err());
    }
    #[test]
    fn ordering_and_formatter_validation_reject_false_receipt_shapes() {
        let mut fixture = Fixture::new();
        fixture.0.execution.network_steps = vec![
            NetworkStep {
                stage: Stage::Firmware,
                step: 0,
            },
            NetworkStep {
                stage: Stage::Carapace,
                step: 1,
            },
        ];
        assert!(Inventory::from_document(fixture.0).is_err());
        let mut fixture = Fixture::new();
        fixture.0.inputs[0].roles = vec![Role::CarapaceBase, Role::CarapaceBase];
        assert!(Inventory::from_document(fixture.0).is_err());
        let mut fixture = Fixture::new();
        let mut formatter = Fixture::formatter();
        formatter.runtime.reverse();
        fixture.0.execution.formatters.push(formatter);
        assert!(Inventory::from_document(fixture.0).is_err());
    }
}
