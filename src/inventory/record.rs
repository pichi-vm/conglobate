// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{Inventory, InventoryError, schema::*};
use crate::{
    output::AssembledImage, pipeline::InheritedBuild, recipe::DEFAULT_TRANSFORM,
    request::BuildRequest,
};
use sha2::{Digest as _, Sha256};
use std::{fs::File, io::Read, path::Path};
impl Digest {
    fn from_bytes(bytes: &[u8]) -> Self {
        Self {
            algorithm: Algorithm::Sha256,
            digest: Sha256::digest(bytes).into(),
        }
    }
    fn identity(bytes: &[u8]) -> Result<Self, InventoryError> {
        Ok(Self {
            algorithm: Algorithm::Sha256,
            digest: bytes.try_into().map_err(InventoryError::invalid)?,
        })
    }
    fn oci(value: &str) -> Result<Self, InventoryError> {
        let digest: pichi_artifact::Digest = value.parse().map_err(InventoryError::invalid)?;
        Self::identity(digest.as_bytes())
    }
}
impl Builder {
    fn current() -> Result<Self, InventoryError> {
        let mut executable = File::open("/proc/self/exe").map_err(InventoryError::invalid)?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let n = executable
                .read(&mut buffer)
                .map_err(InventoryError::invalid)?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        Ok(Self {
            name: "conglobate".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            executable: Digest {
                algorithm: Algorithm::Sha256,
                digest: hash.finalize().into(),
            },
        })
    }
}
impl SelectedDocument {
    fn captured(document: &crate::input::SelectedDocument) -> Result<Self, InventoryError> {
        Ok(Self {
            path: Context::text(document.path().as_ref())?,
            content: Digest::identity(document.sha256())?,
        })
    }
}
impl Context {
    fn text(path: &Path) -> Result<String, InventoryError> {
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| InventoryError::invalid("inventory path is not UTF-8"))
    }
}
impl Outputs {
    fn inherited(output: &AssembledImage) -> Result<Self, InventoryError> {
        let mut pmi = None;
        let mut dtb = None;
        for layer in &output.image().manifest().layers {
            match layer {
                pichi_artifact::Layer::Pmi(d) => pmi = Some(Digest::oci(&d.digest)?),
                pichi_artifact::Layer::Dtb(d) => dtb = Some(Digest::oci(&d.digest)?),
                _ => {}
            }
        }
        Ok(Self {
            manifest: Digest::identity(output.image().manifest_bytes().digest().as_bytes())?,
            carapace_root: output.chain().map(|c| c.root().to_vec().into()),
            scutes: output
                .chain()
                .into_iter()
                .flat_map(|c| c.scutes())
                .map(|s| {
                    Ok(Scute {
                        digest: Digest::identity(s.digest().as_bytes())?,
                        origin: Origin::Inherited,
                    })
                })
                .collect::<Result<_, InventoryError>>()?,
            pmi,
            dtb,
        })
    }
}
impl Inventory {
    pub(crate) fn inherited(
        request: &BuildRequest,
        completed: &InheritedBuild<'_>,
        output: &AssembledImage,
        transformed: &crate::transform::TransformedDtb,
    ) -> Result<Self, InventoryError> {
        Self::from_build(request, completed.build(), output, transformed, &[])
    }
    pub(crate) fn from_build(
        request: &BuildRequest,
        build: &crate::input::ResolvedBuild,
        output: &AssembledImage,
        transformed: &crate::transform::TransformedDtb,
        produced: &[std::sync::Arc<crate::storage::SealedScute<'_>>],
    ) -> Result<Self, InventoryError> {
        let authored = matches!(
            build.captured().plan().recipe().transform(),
            crate::recipe::TransformSelection::Authored(_)
        );
        let mut outputs = Outputs::inherited(output)?;
        let start = outputs
            .scutes
            .len()
            .checked_sub(produced.len())
            .ok_or_else(|| InventoryError::invalid("missing produced output scutes"))?;
        for (entry, scute) in outputs.scutes[start..].iter_mut().zip(produced) {
            if entry.digest.digest != Digest::oci(&scute.descriptor().digest)?.digest {
                return Err(InventoryError::invalid(
                    "produced scute differs from output",
                ));
            }
            entry.origin = Origin::Produced;
        }
        let captured = build.captured();
        let context = captured.context();
        let mut inputs = Vec::new();
        for input in captured.plan().inputs() {
            let image = build
                .images()
                .get(input.reference())
                .ok_or_else(|| InventoryError::invalid("unresolved inventory input"))?;
            let roles = input
                .roles()
                .iter()
                .map(|role| match role {
                    crate::plan::InputRole::BootstrapTools => Role::BootstrapTools,
                    crate::plan::InputRole::CarapaceBase => Role::CarapaceBase,
                    crate::plan::InputRole::FirmwareTools => Role::FirmwareTools,
                })
                .collect();
            inputs.push(Input {
                roles,
                reference: input.reference().as_ref().into(),
                index: Digest::identity(input.index().bytes())?,
                manifest: Digest::identity(image.image().manifest_bytes().digest().as_bytes())?,
                carapace_root: image.chain().map(|c| c.root().to_vec().into()),
            });
        }
        Self::from_document(Document {
            version: 2,
            builder: Builder::current()?,
            context: Context {
                repository: Context::text(request.repository())?,
                object_format: match context.object_format() {
                    gix::hash::Kind::Sha1 => ObjectFormat::Sha1,
                    gix::hash::Kind::Sha256 => ObjectFormat::Sha256,
                    _ => return Err(InventoryError::invalid("unsupported Git object format")),
                },
                commit: context.commit().as_bytes().to_vec().into(),
                tree: context.subtree().as_bytes().to_vec().into(),
                subdirectory: Context::text(request.context().as_ref())?,
            },
            inputs,
            execution: Execution {
                architecture: match output.image().architecture() {
                    crate::cache::HostArchitecture::Amd64 => Architecture::Amd64,
                    crate::cache::HostArchitecture::Arm64 => Architecture::Arm64,
                },
                recipe: SelectedDocument::captured(captured.recipe_document())?,
                lock: SelectedDocument::captured(captured.lock_document())?,
                transform: Transform {
                    source: if authored {
                        TransformSource::Authored
                    } else {
                        TransformSource::Builtin
                    },
                    digest: if authored {
                        Digest::identity(captured.recipe_document().sha256())?
                    } else {
                        Digest::from_bytes(DEFAULT_TRANSFORM.as_bytes())
                    },
                    target_present: transformed.bytes().is_some(),
                    default_skipped: transformed.default_skipped(),
                },
                formatters: vec![],
                network_steps: vec![],
            },
            outputs,
        })
    }
    pub(crate) fn check_output(&self, output: &AssembledImage) -> Result<(), InventoryError> {
        let expected = Outputs::inherited(output)?;
        // Origins are checked by the producing stage; byte identities and
        // chain order must agree independently with the assembled output.
        let actual = &self.document.outputs;
        let same_digest = |a: &Option<Digest>, b: &Option<Digest>| {
            a.as_ref().map(|d| d.digest) == b.as_ref().map(|d| d.digest)
        };
        if actual.manifest.digest != expected.manifest.digest
            || actual.carapace_root != expected.carapace_root
            || !same_digest(&actual.pmi, &expected.pmi)
            || !same_digest(&actual.dtb, &expected.dtb)
            || actual
                .scutes
                .iter()
                .map(|s| s.digest.digest)
                .ne(expected.scutes.iter().map(|s| s.digest.digest))
        {
            return Err(InventoryError::invalid(
                "inventory output identities disagree with assembled image",
            ));
        }
        Ok(())
    }
}
