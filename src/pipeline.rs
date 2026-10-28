// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

pub(crate) mod artifacts;
mod bootstrap;
mod carapace;
mod firmware;
mod output;
mod rejection;
use crate::{
    cache::Cache,
    input::{CapturedBuild, ResolvedBuild},
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
    /// Resolve local inputs, execute the plan and publish its verified output.
    /// The caller supplies the dedicated disposable-VM environment; this method
    /// does not manage VM shutdown or turn the calling process into a sandbox.
    ///
    /// # Errors
    /// Rejects non-root execution, invalid/unavailable inputs, unsupported work,
    /// failed stage execution, cleanup, assembly or publication.
    /// Ordinary errors do not authorize another build in the same VM. Some
    /// failures retain unfinished kernel resources until environment disposal;
    /// returning an error is not a successful teardown receipt.
    pub(crate) fn run(self) -> anyhow::Result<()> {
        if !rustix::process::geteuid().is_root() {
            return Err(anyhow!("conglobate requires root"));
        }
        let build =
            CapturedBuild::try_from(&self.request)?.resolve(Cache::open(self.request.cache())?)?;
        let inherited = !build.captured().graph().execution_required();
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
        use crate::plan::{Reference, TaskKind};
        use anyhow::Context;
        use std::collections::BTreeMap;
        let plan = build.captured().graph().plan();
        let mut outputs = BTreeMap::new();
        let mut artifacts = artifacts::Artifacts::new();
        let mut reaper = crate::process::ChildReaper::install()?;
        let selected = match plan.image() {
            crate::plan::Image::Short(name) => name,
            crate::plan::Image::Long { root, .. } => root,
        };
        for name in plan.order() {
            let task = &plan.tasks()[name];
            // DESIGN §7: only a completed selected output supplies CARAPACE.
            // Input, ancestor and tool identities cannot stand in for a root
            // that this task has not produced yet, including local tool roots.
            let carapace = outputs.get(selected).and_then(StageOutput::root);
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
                        .execute(
                            workspace,
                            runtime,
                            active,
                            reaper,
                            carapace.as_ref(),
                        )?;
                        Ok((
                            StageOutput {
                                base: None,
                                produced: vec![result.scute],
                                firmware: None,
                                boot_base: None,
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
                                carapace.as_ref(),
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
                        let result = firmware::BuildTools {
                            active,
                            root: carapace,
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
        StageOutput::select(plan, &outputs)
    }
}
/// Created only after the selected plan completed and its output was verified.
#[derive(Debug)]
pub(crate) struct CompletedImage {
    image: AssembledImage,
}
impl CompletedImage {
    pub(crate) fn publish(self) -> anyhow::Result<()> {
        self.image.publish()?;
        Ok(())
    }
}
