// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Version-one input/output inventory and RFC 8949 core deterministic encoding.

use crate::Error;
use ciborium::Value;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Digest {
    pub algorithm: String,
    #[serde(with = "serde_bytes")]
    pub digest: Vec<u8>,
}

impl Digest {
    pub fn sha256(bytes: &[u8]) -> Self {
        Self {
            algorithm: "sha256".into(),
            digest: Sha256::digest(bytes).to_vec(),
        }
    }
    pub fn from_oci(text: &str) -> Result<Self, Error> {
        crate::recipe::IndexDigest::try_from(text.to_owned())?;
        Ok(Self {
            algorithm: "sha256".into(),
            digest: hex::decode(&text[7..]).map_err(invalid)?,
        })
    }
    pub fn oci(&self) -> String {
        format!("{}:{}", self.algorithm, hex::encode(&self.digest))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Builder {
    pub name: String,
    pub version: String,
    pub executable: Digest,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub repository: String,
    pub object_format: String,
    #[serde(with = "serde_bytes")]
    pub commit: Vec<u8>,
    pub subdirectory: String,
    #[serde(with = "serde_bytes")]
    pub tree: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub roles: Vec<String>,
    pub reference: String,
    pub index: Digest,
    pub manifest: Digest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carapace_root: Option<serde_bytes::ByteBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkStep {
    pub recipe: String,
    pub step: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    pub source: String,
    pub digest: Digest,
    pub target_present: bool,
    /// True when the implicit default skipped the missing DTB or bootargs.
    pub default_skipped: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub architecture: String,
    /// Includes refs.lock and every authored stage file, including empty files.
    pub recipes: BTreeMap<String, Digest>,
    pub transform: Transform,
    pub network_steps: Vec<NetworkStep>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scute {
    pub digest: Digest,
    pub origin: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outputs {
    pub manifest: Digest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carapace_root: Option<serde_bytes::ByteBuf>,
    pub scutes: Vec<Scute>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmi: Option<Digest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtb: Option<Digest>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Inventory {
    pub version: u32,
    pub builder: Builder,
    pub context: Context,
    pub inputs: Vec<Input>,
    pub execution: Execution,
    pub outputs: Outputs,
}

impl Inventory {
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut value = Value::serialized(self).map_err(invalid)?;
        canonicalize(&mut value)?;
        let mut bytes = Vec::new();
        ciborium::into_writer(&value, &mut bytes).map_err(invalid)?;
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, Error> {
        let mut input = bytes;
        let inventory: Self = ciborium::from_reader(&mut input).map_err(invalid)?;
        if !input.is_empty() {
            return Err(invalid("trailing inventory data"));
        }
        if inventory.encode()? != bytes {
            return Err(invalid("inventory is not core deterministic CBOR"));
        }
        Ok(inventory)
    }

    fn validate(&self) -> Result<(), Error> {
        if self.version != 1 {
            return Err(invalid("unsupported inventory version"));
        }
        if self.builder.name != "conglobate" {
            return Err(invalid("unexpected inventory builder"));
        }
        if !matches!(self.execution.architecture.as_str(), "amd64" | "arm64") {
            return Err(invalid("unsupported inventory architecture"));
        }
        if !matches!(
            self.execution.transform.source.as_str(),
            "builtin" | "authored"
        ) || (self.execution.transform.source == "authored"
            && self.execution.transform.default_skipped)
        {
            return Err(invalid("invalid transform disposition"));
        }
        if !self.execution.recipes.contains_key("refs.lock") {
            return Err(invalid("inventory is missing refs.lock"));
        }
        if self.outputs.scutes.is_empty() != self.outputs.carapace_root.is_none() {
            return Err(invalid("output scutes and root must occur together"));
        }
        if self
            .outputs
            .scutes
            .iter()
            .any(|scute| !matches!(scute.origin.as_str(), "inherited" | "produced"))
        {
            return Err(invalid("unknown scute origin"));
        }
        if self.execution.network_steps.iter().any(|step| {
            !matches!(
                step.recipe.as_str(),
                "bootstrap.yaml" | "carapace.yaml" | "firmware.yaml"
            )
        }) || self
            .execution
            .network_steps
            .windows(2)
            .any(|w| (&w[0].recipe, w[0].step) >= (&w[1].recipe, w[1].step))
        {
            return Err(invalid(
                "network steps must be unique and in pipeline order",
            ));
        }
        let context_len = match self.context.object_format.as_str() {
            "sha1" => 20,
            "sha256" => 32,
            _ => return Err(invalid("unknown Git object format")),
        };
        if self.context.commit.len() != context_len || self.context.tree.len() != context_len {
            return Err(invalid("invalid Git object identity length"));
        }
        let mut digests = vec![
            &self.builder.executable,
            &self.execution.transform.digest,
            &self.outputs.manifest,
        ];
        digests.extend(self.execution.recipes.values());
        digests.extend(self.outputs.scutes.iter().map(|s| &s.digest));
        digests.extend(self.outputs.pmi.iter());
        digests.extend(self.outputs.dtb.iter());
        for input in &self.inputs {
            digests.extend([&input.index, &input.manifest]);
            if input.roles.iter().any(|role| {
                !matches!(
                    role.as_str(),
                    "bootstrap-tools" | "carapace-base" | "firmware-tools"
                )
            }) {
                return Err(invalid("unknown input role"));
            }
            if input
                .carapace_root
                .as_ref()
                .is_some_and(|root| root.len() != 32)
                || (input.carapace_root.is_none()
                    && input.roles.iter().any(|role| role != "carapace-base"))
            {
                return Err(invalid("invalid input root length"));
            }
            if input.roles.is_empty() || input.roles.windows(2).any(|w| w[0] >= w[1]) {
                return Err(invalid("input roles must be nonempty, sorted and unique"));
            }
        }
        if digests
            .iter()
            .any(|d| d.algorithm != "sha256" || d.digest.len() != 32)
        {
            return Err(invalid("invalid inventory digest"));
        }
        if self
            .inputs
            .windows(2)
            .any(|w| w[0].reference >= w[1].reference)
        {
            return Err(invalid("inventory inputs must be sorted and unique"));
        }
        if self
            .outputs
            .carapace_root
            .as_ref()
            .is_some_and(|root| root.len() != 32)
        {
            return Err(invalid("invalid output root length"));
        }
        Ok(())
    }
}

fn invalid(error: impl std::fmt::Display) -> Error {
    Error::Invalid(format!("inventory: {error}"))
}

fn canonicalize(value: &mut Value) -> Result<(), Error> {
    match value {
        Value::Map(entries) => {
            let mut sorted = Vec::with_capacity(entries.len());
            for (key, value) in std::mem::take(entries) {
                let mut key = key;
                let mut value = value;
                canonicalize(&mut key)?;
                canonicalize(&mut value)?;
                let mut encoded = Vec::new();
                ciborium::into_writer(&key, &mut encoded).map_err(invalid)?;
                sorted.push((encoded, key, value));
            }
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            if sorted.windows(2).any(|w| w[0].0 == w[1].0) {
                return Err(invalid("duplicate map key"));
            }
            entries.extend(sorted.into_iter().map(|(_, key, value)| (key, value)));
        }
        Value::Array(entries) => {
            for value in entries {
                canonicalize(value)?;
            }
        }
        Value::Float(_) | Value::Tag(_, _) => {
            return Err(invalid("floats and tags are not part of inventory v1"));
        }
        _ => {}
    }
    Ok(())
}

/// Trusted finalized build state used to construct the emitted inventory.
#[derive(Debug)]
pub struct Record<'a> {
    pub inputs: &'a crate::input::Inputs,
    pub output: &'a pichi_artifact::Manifest,
    pub repository: &'a std::path::Path,
    pub subdirectory: &'a std::path::Path,
    pub before_transform: Option<&'a [u8]>,
    /// Final chain order; manifest layer ordering is not load-bearing.
    pub scute_order: &'a [String],
}

impl Record<'_> {
    pub fn inventory(&self) -> Result<Inventory, Error> {
        use crate::recipe::Directive;
        use pichi_artifact::Layer;
        let source = &self.inputs.context;
        let pipeline = &self.inputs.pipeline;
        let mut recipes = BTreeMap::new();
        for name in [
            "bootstrap.yaml",
            "carapace.yaml",
            "firmware.yaml",
            "transform.yaml",
            "refs.lock",
        ] {
            if let Some(text) = source.recipe(name)? {
                recipes.insert(name.into(), Digest::sha256(text.as_bytes()));
            }
        }
        let authored = source.recipe("transform.yaml")?;
        let transform = authored.unwrap_or(crate::transform::DEFAULT_YAML);
        let default_skipped = if authored.is_some() {
            false
        } else {
            match self.before_transform {
                None => true,
                Some(bytes) => {
                    let tree: devtree::Tree<'_> = devtree::Tree::parse(bytes).map_err(invalid)?;
                    let tree = devtree::OwnedTree::from(&tree);
                    tree.find_path("/chosen")
                        .and_then(|node| node.property("bootargs"))
                        .is_none()
                }
            }
        };
        let mut network_steps = Vec::new();
        for (recipe, directives) in [
            (
                "bootstrap.yaml",
                pipeline.bootstrap.as_ref().map(|b| &b.derive),
            ),
            (
                "carapace.yaml",
                pipeline.carapace.as_ref().map(|c| &c.derive),
            ),
            (
                "firmware.yaml",
                pipeline.firmware.as_ref().map(|f| &f.derive),
            ),
        ] {
            if let Some(directives) = directives {
                for (step, directive) in directives.iter().enumerate() {
                    if matches!(directive, Directive::Run { network: true, .. }) {
                        network_steps.push(NetworkStep {
                            recipe: recipe.into(),
                            step: step as u64,
                        });
                    }
                }
            }
        }
        let mut inputs = Vec::new();
        for (reference, image) in &self.inputs.images {
            let mut roles = Vec::new();
            if pipeline
                .bootstrap
                .as_ref()
                .is_some_and(|b| &b.from == reference)
            {
                roles.push("bootstrap-tools".into());
            }
            if pipeline.base() == Some(reference) {
                roles.push("carapace-base".into());
            }
            if pipeline.firmware.as_ref().and_then(|f| f.from.as_ref()) == Some(reference) {
                roles.push("firmware-tools".into());
            }
            inputs.push(Input {
                roles,
                reference: reference.as_ref().into(),
                index: Digest::from_oci(&image.index)?,
                manifest: Digest::from_oci(&image.manifest_digest)?,
                carapace_root: image
                    .manifest
                    .carapace_verity_hash()
                    .map(|root| root.to_vec().into()),
            });
        }
        let inherited: std::collections::BTreeSet<_> = pipeline
            .base()
            .and_then(|reference| self.inputs.images.get(reference))
            .into_iter()
            .flat_map(|image| image.manifest.layers.iter())
            .filter(|layer| matches!(layer, Layer::Scute(_) | Layer::ScuteZstd(_)))
            .map(Layer::digest_str)
            .collect();
        let mut scutes = Vec::new();
        for digest in self.scute_order {
            scutes.push(Scute {
                digest: Digest::from_oci(digest)?,
                origin: if inherited.contains(digest.as_str()) {
                    "inherited"
                } else {
                    "produced"
                }
                .into(),
            });
        }
        let firmware_digest = |pmi: bool| {
            self.output
                .layers
                .iter()
                .find(|layer| {
                    if pmi {
                        matches!(layer, Layer::Pmi(_))
                    } else {
                        matches!(layer, Layer::Dtb(_))
                    }
                })
                .map(|layer| Digest::from_oci(layer.digest_str()))
                .transpose()
        };
        let (executable, _) = crate::scute::digest_file(std::path::Path::new("/proc/self/exe"))?;
        Ok(Inventory {
            version: 1,
            builder: Builder {
                name: "conglobate".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                executable: Digest::from_oci(&executable)?,
            },
            context: Context {
                repository: self.repository.to_string_lossy().into_owned(),
                object_format: source.object_format.clone(),
                commit: hex::decode(&source.commit).map_err(invalid)?,
                subdirectory: self.subdirectory.to_string_lossy().into_owned(),
                tree: hex::decode(&source.tree).map_err(invalid)?,
            },
            inputs,
            execution: Execution {
                architecture: crate::cache::host_architecture()?.into(),
                recipes,
                transform: Transform {
                    source: if authored.is_some() {
                        "authored"
                    } else {
                        "builtin"
                    }
                    .into(),
                    digest: Digest::sha256(transform.as_bytes()),
                    target_present: self.before_transform.is_some(),
                    default_skipped,
                },
                network_steps,
            },
            outputs: Outputs {
                manifest: Digest::sha256(&self.output.to_bytes().map_err(invalid)?),
                carapace_root: self
                    .output
                    .carapace_verity_hash()
                    .map(|root| root.to_vec().into()),
                scutes,
                pmi: firmware_digest(true)?,
                dtb: firmware_digest(false)?,
            },
        })
    }
}
