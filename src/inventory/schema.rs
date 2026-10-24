// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Private wire records. Only validated, immutable Inventory values are public.
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Document {
    pub(crate) version: u32,
    pub(crate) builder: Builder,
    pub(crate) context: Context,
    pub(crate) inputs: Vec<Input>,
    pub(crate) execution: Execution,
    pub(crate) outputs: Outputs,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Digest {
    pub(crate) algorithm: Algorithm,
    #[serde(with = "serde_bytes")]
    pub(crate) digest: [u8; 32],
}
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum Algorithm {
    #[serde(rename = "sha256")]
    Sha256,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Builder {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) executable: Digest,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Context {
    pub(crate) repository: String,
    pub(crate) object_format: ObjectFormat,
    pub(crate) commit: ByteBuf,
    pub(crate) subdirectory: String,
    pub(crate) tree: ByteBuf,
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
    pub(crate) roles: Vec<Role>,
    pub(crate) reference: String,
    pub(crate) index: Digest,
    pub(crate) manifest: Digest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) carapace_root: Option<ByteBuf>,
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
    pub(crate) stage: Stage,
    pub(crate) step: u64,
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
    pub(crate) source: TransformSource,
    pub(crate) digest: Digest,
    pub(crate) target_present: bool,
    pub(crate) default_skipped: bool,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SelectedDocument {
    pub(crate) path: String,
    pub(crate) content: Digest,
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
    pub(crate) id_mappings: Option<IdMappings>,
    pub(crate) architecture: Architecture,
    pub(crate) recipe: SelectedDocument,
    pub(crate) lock: SelectedDocument,
    pub(crate) transform: Transform,
    pub(crate) formatters: Vec<Formatter>,
    pub(crate) network_steps: Vec<NetworkStep>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RuntimeFile {
    pub(crate) path: String,
    pub(crate) digest: Digest,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Filesystem {
    #[serde(rename = "type")]
    pub(crate) kind: FilesystemType,
    pub(crate) size_bytes: u64,
    pub(crate) logical_size_bytes: u64,
    pub(crate) label: String,
}
#[derive(Debug, Serialize, Deserialize)]
pub(super) enum FilesystemType {
    #[serde(rename = "ext4")]
    Ext4,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Formatter {
    pub(crate) stage: Stage,
    pub(crate) profile: String,
    pub(crate) profile_digest: Digest,
    pub(crate) executable: RuntimeFile,
    pub(crate) runtime: Vec<RuntimeFile>,
    pub(crate) configuration: Digest,
    pub(crate) arguments: Vec<String>,
    pub(crate) filesystem: Filesystem,
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
    pub(crate) digest: Digest,
    pub(crate) origin: Origin,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Outputs {
    pub(crate) manifest: Digest,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) carapace_root: Option<ByteBuf>,
    pub(crate) scutes: Vec<Scute>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pmi: Option<Digest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dtb: Option<Digest>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IdMappings {
    pub(crate) uid: IdRange,
    pub(crate) gid: IdRange,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IdRange {
    pub(crate) parent_start: u32,
    pub(crate) count: u32,
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
