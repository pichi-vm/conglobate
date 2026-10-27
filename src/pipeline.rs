// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Recipe orchestration. Unsupported execution paths fail before publication.
pub(crate) mod artifacts;
mod bootstrap;
mod carapace;
mod firmware;
mod output;
mod rejection;
use crate::{
    cache::Cache,
    input::{CapturedBuild, ResolvedBuild},
    inventory::Inventory,
    output::AssembledImage,
    request::BuildRequest,
    storage::{Runtime, Workspace},
};
use output::StageOutput;

use anyhow::anyhow;

#[derive(Debug)]
pub(crate) struct Pipeline {
    request: BuildRequest,
}
impl Pipeline {
    /// Retain a parsed request without resolving inputs or acquiring resources.
    pub(crate) fn new(request: BuildRequest) -> Self {
        Self { request }
    }
    /// Resolve local inputs, execute the recipe and publish its verified output.
    /// The caller supplies the dedicated disposable-VM environment; this method
    /// does not manage VM shutdown or turn the calling process into a sandbox.
    ///
    /// # Errors
    /// Rejects non-root execution, invalid/unavailable inputs, unsupported work,
    /// failed stage execution, cleanup, assembly, inventory or publication.
    /// Ordinary errors do not authorize another build in the same VM. Some
    /// failures retain unfinished kernel resources until environment disposal;
    /// returning an error is not a successful teardown receipt.
    pub(crate) fn run(self) -> anyhow::Result<()> {
        if !rustix::process::geteuid().is_root() {
            return Err(anyhow!("conglobate requires root"));
        }
        let build =
            CapturedBuild::try_from(&self.request)?.resolve(Cache::open(self.request.cache())?)?;
        let inherited = !build.captured().plan().execution_required();
        let output = if inherited {
            StageOutput::inherited(&build)?.assemble(&self.request, &build)?
        } else {
            let workspace = Workspace::create()?;
            let stages = Runtime::scoped(&workspace, std::path::Path::new("/run"), |runtime| {
                self.execute(&build, &workspace, runtime)
            })?;
            stages.assemble(&self.request, &build)?
        };
        output.publish()?;
        Ok(())
    }
    fn execute<'b, 'w>(
        &self,
        build: &'b ResolvedBuild,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
    ) -> anyhow::Result<StageOutput<'b, 'w>> {
        use crate::recipe::{Reference, TaskKind};
        use anyhow::Context;
        use std::collections::BTreeMap;
        let recipe = build.captured().plan().recipe();
        let mut outputs = BTreeMap::new();
        let mut artifacts = artifacts::Artifacts::new();
        let mut receipts = Vec::new();
        let mut reaper = crate::process::ChildReaper::install()?;
        for name in recipe.order() {
            let task = &recipe.tasks()[name];
            let input = match task.from() {
                Reference::Remote(remote) => {
                    let reference = remote.as_ref().parse()?;
                    StageOutput::remote(
                        build
                            .images()
                            .get(&reference)
                            .ok_or(anyhow!("unresolved task input"))?,
                    )
                }
                Reference::Local(local) => outputs
                    .get(local)
                    .cloned()
                    .ok_or(anyhow!("task input not completed"))?,
            };
            let (result, next_reaper) = (|| -> anyhow::Result<_> {
                match task.kind() {
                    TaskKind::Bootstrap(stage) => {
                        let active = input.activate(build, workspace)?;
                        let result = bootstrap::BootstrapBuild::new(
                            build,
                            stage,
                            task.directives(),
                            &artifacts,
                        )
                        .execute(workspace, runtime, active, reaper)?;
                        receipts.push(result.receipt);
                        Ok((
                            StageOutput {
                                base: None,
                                produced: vec![result.scute],
                                firmware: None,
                                boot_base: None,
                                formatters: vec![],
                            },
                            result.reaper,
                        ))
                    }
                    TaskKind::Extend => {
                        let mut output = input;
                        if !task.directives().is_empty() {
                            let active = output.activate(build, workspace)?;
                            let derived = carapace::DerivedCarapace {
                                active,
                                scutes: output.produced,
                                reaper,
                            }
                            .derive(
                                build,
                                workspace,
                                runtime,
                                task.directives(),
                                &artifacts,
                            )?;
                            derived.active.close()?;
                            output.produced = derived.scutes;
                            output.rewrite(build, workspace)?;
                            Ok((output, derived.reaper))
                        } else {
                            Ok((output, reaper))
                        }
                    }
                    TaskKind::Build => {
                        let active = input.activate(build, workspace)?;
                        // CARAPACE is available only from a local produced output,
                        // never the remote tool carapace. Dependencies execute first.
                        let selected = match recipe.image() {
                            crate::recipe::Image::Short(name) => name,
                            crate::recipe::Image::Long { root, .. } => root,
                        };
                        let root =
                            outputs
                                .get(selected)
                                .and_then(StageOutput::root)
                                .or_else(|| {
                                    if matches!(task.from(), Reference::Local(_)) {
                                        input.root()
                                    } else {
                                        None
                                    }
                                });
                        let result = firmware::BuildTools {
                            active,
                            root,
                            reaper,
                        }
                        .run(
                            build,
                            task,
                            workspace,
                            runtime,
                            &mut artifacts,
                        )?;
                        Ok((
                            StageOutput {
                                base: None,
                                produced: vec![],
                                firmware: result.firmware,
                                boot_base: None,
                                formatters: vec![],
                            },
                            result.reaper,
                        ))
                    }
                }
            })()
            .with_context(|| format!("task {name}"))?;
            reaper = next_reaper;
            outputs.insert(name.clone(), result);
        }
        let mut output = StageOutput::select(recipe, &outputs)?;
        output.formatters = receipts;
        Ok(output)
    }
}
/// Created only after the selected recipe path completed and inventory binds
/// its finalized output. Neither decoded inventory nor raw storage can create it.
#[derive(Debug)]
pub(crate) struct CompletedImage {
    image: AssembledImage,
    inventory: Inventory,
}
impl CompletedImage {
    pub(crate) fn publish(self) -> anyhow::Result<()> {
        self.image.publish_inventory(&self.inventory)?;
        Ok(())
    }
}
