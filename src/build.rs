// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Standalone assembly line. All execution storage lives in one private tmpfs.

use crate::{
    Error,
    artifact::Artifact,
    cache::Cache,
    execute::{Step, private_mounts},
    input::Inputs,
    inventory::Record,
    mounts::{Filesystem, Scratch},
    paths::resolve,
    recipe::Directive,
    scute::{Chain, Scute},
    snapshot::Snapshot,
    template::Template,
};
use clap::Args;
use nix::sched::{CloneFlags, unshare};
use pichi_artifact::{ConfigDescriptor, DtbDescriptor, Layer, Manifest, PmiDescriptor};
use std::{
    collections::BTreeMap,
    fs::File,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Args)]
pub struct Build {
    #[arg(long)]
    pub repository: PathBuf,
    #[arg(long)]
    pub commit: String,
    #[arg(long, default_value = ".")]
    pub context: PathBuf,
    #[arg(long)]
    pub cache: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
}

impl Build {
    /// Invoke only in the binary's single-threaded supervisor process.
    pub fn run(&self) -> Result<(), Error> {
        if std::fs::symlink_metadata(&self.output).is_ok() {
            return Err(Error::Invalid("output destination already exists".into()));
        }
        unshare(CloneFlags::CLONE_NEWNS)?;
        private_mounts()?;
        crate::cancel::check()?;
        let scratch = Scratch::new().map_err(|e| Error::Invalid(format!("create scratch: {e}")))?;
        let cache = Cache::new(&self.cache);
        let inputs = Inputs::resolve(&self.repository, &self.commit, &self.context, &cache)
            .map_err(|e| Error::Invalid(format!("resolve inputs: {e}")))?;
        let context = scratch.path().join("context");
        inputs
            .context
            .materialize(&context)
            .map_err(|e| Error::Invalid(format!("checkout context: {e}")))?;
        let artifact = Artifact::new(&scratch.path().join("artifact"))?;
        let mut tools = BTreeMap::new();
        for (index, (reference, image)) in inputs.images.iter().enumerate() {
            tools.insert(
                reference.clone(),
                Chain::stage(
                    &cache,
                    image,
                    &scratch.path().join(format!("input-{index}")),
                )
                .map_err(|e| Error::Invalid(format!("stage input {}: {e}", reference.as_ref())))?,
            );
        }
        // Pin helper bytes from the running executable, not its pathname:
        // replacing an installed binary during a build must not change workers.
        let executable = scratch.path().join("executor");
        std::fs::copy("/proc/self/exe", &executable)?;
        let environment = Environment {
            workspace: scratch.path(),
            context: &context,
            executable,
        };
        let mut manifest = empty_manifest();
        let mut chain = None;
        if let Some(bootstrap) = &inputs.pipeline.bootstrap {
            let tool = &tools[&bootstrap.from];
            let (snapshot, filesystem) = environment.mount(tool, "bootstrap")?;
            environment.derive(&bootstrap.derive, filesystem.path(), None)?;
            let raw = output_file(filesystem.path(), bootstrap.into.as_ref(), None)?;
            let scute = Scute::import(&raw, &scratch.path().join("bootstrap-output"))?;
            filesystem.finish()?;
            drop(snapshot);
            append(&artifact, &mut manifest, &scute)?;
            chain = Some(Chain {
                scutes: vec![scute],
            });
        } else if let Some(reference) = inputs.pipeline.base() {
            let base = &inputs.images[reference];
            artifact
                .inherit(&cache, base)
                .map_err(|e| Error::Invalid(format!("inherit base: {e}")))?;
            manifest = base.manifest.clone();
            chain = tools.remove(reference);
        }
        if let Some(carapace) = &inputs.pipeline.carapace {
            let chain = chain
                .as_mut()
                .ok_or_else(|| Error::Invalid("carapace stage has no input".into()))?;
            for (index, directive) in carapace.derive.iter().enumerate() {
                let (snapshot, filesystem) =
                    environment.mount(chain, &format!("carapace-{index}"))?;
                environment.derive(std::slice::from_ref(directive), filesystem.path(), None)?;
                filesystem.finish()?;
                let scute = snapshot
                    .finish()
                    .map_err(|e| Error::Invalid(format!("seal carapace layer {index}: {e}")))?;
                append(&artifact, &mut manifest, &scute)?;
                chain.scutes.push(scute);
            }
        }
        let root = chain
            .as_ref()
            .map(Chain::root)
            .transpose()?
            .map(hex::encode);
        if let Some(firmware) = &inputs.pipeline.firmware {
            let tool = match &firmware.from {
                Some(reference) => tools.get(reference),
                None => chain.as_ref(),
            };
            // An explicit from always refers to the original locked image,
            // even when that reference also supplies the output's base.
            let explicit_base;
            let tool = if let Some(tool) = tool {
                tool
            } else if let Some(reference) = &firmware.from {
                explicit_base = Chain::stage(
                    &cache,
                    &inputs.images[reference],
                    &scratch.path().join("firmware-input"),
                )?;
                &explicit_base
            } else {
                return Err(Error::Invalid("firmware stage has no tool carapace".into()));
            };
            let (snapshot, filesystem) = environment.mount(tool, "firmware")?;
            environment.derive(&firmware.derive, filesystem.path(), root.as_deref())?;
            let pmi = output_file(
                filesystem.path(),
                firmware.into.pmi.as_ref(),
                root.as_deref(),
            )?;
            let dtb = output_file(
                filesystem.path(),
                firmware.into.dtb.as_ref(),
                root.as_deref(),
            )?;
            let pmi_meta = std::fs::metadata(&pmi)?;
            let dtb_meta = std::fs::metadata(&dtb)?;
            if pmi_meta.dev() == dtb_meta.dev() && pmi_meta.ino() == dtb_meta.ino() {
                return Err(Error::Invalid(
                    "firmware outputs resolve to the same file".into(),
                ));
            }
            let (pmi, pmi_size) = artifact.put(File::open(pmi)?)?;
            let (dtb, dtb_size) = artifact.put(File::open(dtb)?)?;
            filesystem.finish()?;
            drop(snapshot);
            manifest
                .layers
                .retain(|layer| !matches!(layer, Layer::Pmi(_) | Layer::Dtb(_)));
            manifest.layers.extend([
                Layer::Pmi(PmiDescriptor {
                    digest: pmi.oci(),
                    size: pmi_size,
                }),
                Layer::Dtb(DtbDescriptor {
                    digest: dtb.oci(),
                    size: dtb_size,
                }),
            ]);
        }
        let before = manifest
            .layers
            .iter()
            .find(|layer| matches!(layer, Layer::Dtb(_)))
            .map(|layer| {
                std::fs::read(artifact.blob_path(layer.digest_str())?).map_err(Error::from)
            })
            .transpose()?;
        let transformed = inputs
            .pipeline
            .transform
            .apply(before.as_deref(), root.as_deref())?;
        if let Some(bytes) = transformed {
            if let Some(before) = &before {
                let pmi = manifest
                    .layers
                    .iter()
                    .find(|layer| matches!(layer, Layer::Pmi(_)))
                    .ok_or_else(|| Error::Invalid("DTB has no firmware".into()))?;
                crate::firmware::validate_transform(
                    &std::fs::read(artifact.blob_path(pmi.digest_str())?)?,
                    before,
                    &bytes,
                )?;
            }
            let (digest, size) = artifact.put(bytes.as_slice())?;
            for layer in &mut manifest.layers {
                if matches!(layer, Layer::Dtb(_)) {
                    *layer = Layer::Dtb(DtbDescriptor {
                        digest: digest.oci(),
                        size,
                    });
                }
            }
        }
        let order: Vec<_> = chain
            .iter()
            .flat_map(|chain| {
                chain
                    .scutes
                    .iter()
                    .map(|scute| scute.descriptor.digest.clone())
            })
            .collect();
        let inventory = Record {
            inputs: &inputs,
            output: &manifest,
            repository: &self.repository,
            subdirectory: &self.context,
            before_transform: before.as_deref(),
            scute_order: &order,
        }
        .inventory()?;
        artifact
            .finalize(&manifest, &inventory)
            .map_err(|e| Error::Invalid(format!("finalize artifact: {e}")))?;
        crate::cancel::check()?;
        artifact
            .publish(&self.output)
            .map_err(|e| Error::Invalid(format!("publish artifact: {e}")))?;
        Ok(())
    }
}

struct Environment<'a> {
    workspace: &'a Path,
    context: &'a Path,
    executable: PathBuf,
}

impl Environment<'_> {
    fn mount(&self, chain: &Chain, name: &str) -> Result<(Snapshot, Filesystem), Error> {
        let origin = chain
            .assemble()
            .map_err(|e| Error::Invalid(format!("{name}: assemble input: {e}")))?;
        let snapshot = Snapshot::create(origin, chain.root()?, &self.workspace.join(name))
            .map_err(|e| Error::Invalid(format!("{name}: create snapshot: {e}")))?;
        let filesystem = Filesystem::mount(
            &snapshot.device_path(),
            &self.workspace.join(format!("{name}-root")),
        )
        .map_err(|e| Error::Invalid(format!("{name}: mount snapshot: {e}")))?;
        Ok((snapshot, filesystem))
    }
    fn derive(
        &self,
        directives: &[Directive],
        root: &Path,
        carapace: Option<&str>,
    ) -> Result<(), Error> {
        for directive in directives {
            crate::cancel::check()?;
            match directive {
                Directive::Run { command, network } => Step {
                    expected_parent: None,
                    root: root.to_owned(),
                    context: self.context.to_owned(),
                    network: *network,
                    carapace: carapace.map(str::to_owned),
                    command: command.clone(),
                }
                .run(&self.executable)?,
                Directive::Copy(copy) => copy.apply(self.context, root, carapace)?,
            }
        }
        Ok(())
    }
}

fn output_file(root: &Path, path: &str, carapace: Option<&str>) -> Result<PathBuf, Error> {
    let path = Template::parse(path, None)?.expand(carapace, None)?;
    let path = resolve(root, Path::new(&path), true)?;
    if !std::fs::metadata(&path)?.is_file() {
        return Err(Error::Invalid("stage output must be a regular file".into()));
    }
    Ok(path)
}

fn empty_manifest() -> Manifest {
    Manifest {
        schema_version: 2,
        media_type: "application/vnd.oci.image.manifest.v1+json".into(),
        artifact_type: "application/vnd.pichi.artifact.v1+json".into(),
        config: ConfigDescriptor::canonical(),
        layers: Vec::new(),
        annotations: BTreeMap::new(),
    }
}

fn append(artifact: &Artifact, manifest: &mut Manifest, scute: &Scute) -> Result<(), Error> {
    let (digest, _) = artifact.put(File::open(scute.data_path())?)?;
    if digest.oci() != scute.descriptor.digest {
        return Err(Error::Invalid(
            "sealed scute changed before packaging".into(),
        ));
    }
    manifest.layers.push(Layer::Scute(scute.descriptor.clone()));
    for (key, value) in [
        ("algo", "sha256"),
        ("data-block-size", "4096"),
        ("hash-block-size", "4096"),
        ("version", "1"),
        ("hash-type", "1"),
    ] {
        manifest
            .annotations
            .insert(format!("dev.pichi.carapace.verity.{key}"), value.into());
    }
    manifest.annotations.insert(
        "dev.pichi.carapace.verity.hash".into(),
        hex::encode(scute.root),
    );
    Ok(())
}
