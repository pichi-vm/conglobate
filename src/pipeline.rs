// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Recipe orchestration. Unsupported execution paths fail before publication.
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
        let recipe = build.captured().plan().recipe();
        // Output ancestry is independent of the firmware's selected tool image.
        let base = recipe
            .carapace()
            .and_then(|stage| stage.from())
            .and_then(|name| build.images().get(name));
        let directives = recipe
            .carapace()
            .map(|stage| stage.directives())
            .unwrap_or(&[]);
        // Output identities do not require live block mappings. Assemble the
        // output chain only when a stage will actually use its filesystem.
        let needs_chain = !directives.is_empty()
            || recipe
                .firmware()
                .is_some_and(|stage| stage.from().is_none());
        let (active, produced, reaper, receipt) = if recipe.bootstrap().is_some() {
            let bootstrap::Bootstrapped {
                scute,
                reaper,
                receipt,
            } = bootstrap::BootstrapBuild::new(build)?.execute(
                &self.request,
                workspace,
                runtime,
            )?;
            let active = if needs_chain {
                Some(crate::storage::ReadOnlyChain::from_root(
                    std::sync::Arc::clone(&scute),
                )?)
            } else {
                None
            };
            (active, vec![scute], reaper, Some(receipt))
        } else {
            let active = base
                .filter(|_| needs_chain)
                .and_then(|image| image.chain())
                .map(|chain| chain.assemble(build.cache(), workspace))
                .transpose()?;
            (
                active,
                Vec::new(),
                crate::process::ChildReaper::install()?,
                None,
            )
        };

        let (active, produced, reaper) = if directives.is_empty() {
            (active, produced, reaper)
        } else {
            let derived = carapace::DerivedCarapace {
                active: active.ok_or(anyhow!("carapace requires a root volume",))?,
                scutes: produced,
                reaper,
            }
            .derive(build, workspace, runtime, directives)?;
            (Some(derived.active), derived.scutes, derived.reaper)
        };
        let root = produced.last().map(|scute| *scute.root()).or_else(|| {
            base.and_then(|image| image.chain())
                .map(|chain| *chain.root())
        });

        let pair = if let Some(stage) = recipe.firmware() {
            let tools = if let Some(reference) = stage.from() {
                if let Some(active) = active {
                    active.close()?;
                }
                build
                    .images()
                    .get(reference)
                    .and_then(|image| image.chain())
                    .ok_or(anyhow!("firmware tools require a root volume",))?
                    .assemble(build.cache(), workspace)?
            } else {
                active.ok_or(anyhow!("missing firmware tools"))?
            };
            Some(
                firmware::FirmwareTools {
                    active: tools,
                    root,
                    reaper,
                }
                .run(build, stage, workspace, runtime)?,
            )
        } else {
            if let Some(active) = active {
                active.close()?;
            }
            None
        };

        Ok(StageOutput {
            base,
            produced,
            firmware: pair,
            formatter: receipt,
        })
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
