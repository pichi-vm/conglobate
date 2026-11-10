// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

pub(crate) mod boot_config;
mod bootstrap;
mod carapace;
pub(crate) mod output;
pub(crate) mod pmi;
mod rejection;
pub(crate) mod root;
pub(crate) mod sources;
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
    /// The caller supplies an isolated execution environment; this method
    /// does not dispose of that environment or sandbox its own process.
    ///
    /// # Errors
    /// Rejects non-root execution, invalid/unavailable inputs, unsupported work,
    /// failed stage execution, cleanup, assembly or publication.
    /// Ordinary errors do not authorize another build in the same execution
    /// environment. Some failures retain unfinished kernel resources until disposal;
    /// returning an error is not a successful teardown receipt.
    pub(crate) fn run(self) -> anyhow::Result<()> {
        if !rustix::process::geteuid().is_root() {
            return Err(anyhow!("conglobate requires root"));
        }
        let build =
            CapturedBuild::try_from(&self.request)?.resolve(Cache::open(self.request.cache())?)?;
        let inherited = !build.captured().graph().execution_required();
        let output = if inherited {
            StageOutput::select(&build, &std::collections::BTreeMap::new(), None)?
                .assemble(&self.request, &build)?
        } else {
            let staged = crate::output::StagedDirectory::create(self.request.output())?;
            let workspace = Workspace::create()?;
            let _runtime_parent =
                crate::storage::application_directory(std::path::Path::new("/run/conglobate"))?;
            let stages = Runtime::scoped(
                &workspace,
                std::path::Path::new("/run/conglobate"),
                |runtime| self.execute(&build, &workspace, runtime, &staged),
            )?;
            stages.assemble_staged(&self.request, &build, staged)?
        };
        output.publish()?;
        Ok(())
    }
    fn execute<'b, 'w>(
        &self,
        build: &'b ResolvedBuild,
        workspace: &'w Workspace,
        runtime: &Runtime<'_>,
        staged: &crate::output::StagedDirectory,
    ) -> anyhow::Result<StageOutput<'b, 'w>> {
        use anyhow::Context;
        use std::collections::BTreeMap;
        let plan = build.captured().graph().plan();
        let mut outputs = BTreeMap::new();
        let mut reaper = crate::process::ChildReaper::install()?;
        for (index, name) in plan.order.iter().enumerate() {
            eprintln!("Building stage {}/{}: {name}", index + 1, plan.order.len());
            let stage = &plan.stages[name];
            let mut input = StageOutput::resolve(&stage.from, build, &outputs)?;
            let source = input
                .root
                .take()
                .ok_or_else(|| anyhow!("stage {name} has no carapace source"))?;
            let sources = sources::Sources {
                outputs: &outputs,
                build,
                workspace,
                runtime,
            };
            let (output, next_reaper) = (|| -> anyhow::Result<_> {
                let active = source.activate(build.cache())?;
                if stage.format.is_some() {
                    let result = bootstrap::BootstrapBuild::new(build, stage, &sources).execute(
                        workspace,
                        runtime,
                        active,
                        reaper,
                        staged.blob_directory()?,
                    )?;
                    Ok((
                        StageOutput {
                            root: Some(root::Root {
                                base: None,
                                produced: vec![result.scute],
                            }),
                            pmi: None,
                            bootconfig: None,
                        },
                        result.reaper,
                    ))
                } else {
                    let derived = carapace::DerivedCarapace {
                        scutes: source.produced,
                        reaper,
                    }
                    .derive(
                        stage,
                        &sources,
                        active,
                        staged.blob_directory()?,
                    )?;
                    input.root = Some(root::Root {
                        base: source.base,
                        produced: derived.scutes,
                    });
                    input.bind()?;
                    Ok((input, derived.reaper))
                }
            })()
            .with_context(|| format!("stage {name}"))?;
            reaper = next_reaper;
            outputs.insert(name.clone(), output);
        }
        StageOutput::select(build, &outputs, Some((workspace, runtime)))
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
