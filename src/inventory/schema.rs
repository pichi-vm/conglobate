// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Private wire records. Only validated, immutable Inventory values are public.
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Document {
    pub version: u32,
    pub builder: Builder,
    pub context: Context,
    pub inputs: Vec<Input>,
    pub execution: Execution,
    pub outputs: Outputs,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Digest {
    pub algorithm: Algorithm,
    #[serde(with = "serde_bytes")]
    pub digest: [u8; 32],
}
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Algorithm {
    #[serde(rename = "sha256")]
    Sha256,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Builder {
    pub name: String,
    pub version: String,
    pub executable: Digest,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Context {
    pub repository: String,
    pub object_format: ObjectFormat,
    pub commit: ByteBuf,
    pub subdirectory: String,
    pub tree: ByteBuf,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum ObjectFormat {
    Sha1,
    Sha256,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Role {
    BootstrapTools,
    CarapaceBase,
    FirmwareTools,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    pub roles: Vec<Role>,
    pub reference: String,
    pub index: Digest,
    pub manifest: Digest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carapace_root: Option<ByteBuf>,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub(super) enum Stage {
    Bootstrap,
    Carapace,
    Firmware,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NetworkStep {
    pub stage: Stage,
    pub step: u64,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(super) enum TransformSource {
    Builtin,
    Authored,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Transform {
    pub source: TransformSource,
    pub digest: Digest,
    pub target_present: bool,
    pub default_skipped: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelectedDocument {
    pub path: String,
    pub content: Digest,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Architecture {
    Amd64,
    Arm64,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Execution {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_mappings: Option<IdMappings>,
    pub architecture: Architecture,
    pub recipe: SelectedDocument,
    pub lock: SelectedDocument,
    pub transform: Transform,
    pub formatters: Vec<Formatter>,
    pub network_steps: Vec<NetworkStep>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuntimeFile {
    pub path: String,
    pub digest: Digest,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Filesystem {
    #[serde(rename = "type")]
    pub kind: FilesystemType,
    pub size_bytes: u64,
    pub logical_size_bytes: u64,
    pub label: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum FilesystemType {
    #[serde(rename = "ext4")]
    Ext4,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Formatter {
    pub stage: Stage,
    pub profile: String,
    pub profile_digest: Digest,
    pub executable: RuntimeFile,
    pub runtime: Vec<RuntimeFile>,
    pub configuration: Digest,
    pub arguments: Vec<String>,
    pub filesystem: Filesystem,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Origin {
    Inherited,
    Produced,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scute {
    pub digest: Digest,
    pub origin: Origin,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Outputs {
    pub manifest: Digest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub carapace_root: Option<ByteBuf>,
    pub scutes: Vec<Scute>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pmi: Option<Digest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dtb: Option<Digest>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IdMappings {
    pub uid: IdRange,
    pub gid: IdRange,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IdRange {
    pub parent_start: u32,
    pub count: u32,
}
impl From<crate::isolation::IdRange> for IdRange {
    fn from(range: crate::isolation::IdRange) -> Self {
        Self {
            parent_start: range.parent_start(),
            count: range.count(),
        }
    }
}
impl From<crate::isolation::IdMappings> for IdMappings {
    fn from(mappings: crate::isolation::IdMappings) -> Self {
        Self {
            uid: mappings.uid().into(),
            gid: mappings.gid().into(),
        }
    }
}
