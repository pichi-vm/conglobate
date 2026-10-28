// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{
    CommandSandbox, CommandStorage, SetupBudget,
    teardown::{Deadline, TEARDOWN_TIMEOUT},
};
use crate::{
    context::CapturedContext,
    plan::{Copy, Directive},
    process::ChildReaper,
};
use std::time::Instant;

#[derive(Debug)]
pub(crate) struct CompletedDirectives<R: CommandStorage> {
    storage: R::Unmounted,
    reaper: ChildReaper,
}
impl<R: CommandStorage> CompletedDirectives<R> {
    pub(crate) fn into_parts(self) -> (R::Unmounted, ChildReaper) {
        (self.storage, self.reaper)
    }
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct DirectiveInputs<'a, 'w> {
    pub(crate) context: &'a CapturedContext,
    pub(crate) artifacts: &'a crate::pipeline::artifacts::Artifacts<'w>,
    pub(crate) carapace: Option<&'a [u8; 32]>,
    pub(crate) network: Option<&'a crate::network::CommandNetworking<'a>>,
}
impl<'l> CommandSandbox<'l> {
    pub(crate) fn directives_with_inputs<R: CommandStorage>(
        &self,
        mut storage: R::Unmounted,
        mut reaper: ChildReaper,
        directives: &[Directive],
        inputs: DirectiveInputs<'_, '_>,
    ) -> anyhow::Result<CompletedDirectives<R>> {
        let DirectiveInputs {
            context,
            artifacts,
            carapace,
            network,
        } = inputs;
        use anyhow::Context;
        anyhow::ensure!(
            network.is_some()
                || !directives
                    .iter()
                    .any(|d| matches!(d, Directive::Run(run) if run.network())),
            "network-enabled directives require captured networking"
        );
        for (index, directive) in directives.iter().enumerate() {
            (storage, reaper) = (|| match directive {
                Directive::Run(run) => {
                    let budget = SetupBudget::begin();
                    let script = match carapace {
                        Some(root) => {
                            format!("export CARAPACE={}\n{}", hex::encode(root), run.command())
                        }
                        None => run.command().to_owned(),
                    };
                    let receiver = self.receiver(script, &budget)?;
                    let prepared = self.prepare_storage::<R>(storage, reaper, &budget)?;
                    let ready = prepared.launch(receiver, budget.deadline())?;
                    if run.network() {
                        let network = match self.prepare_network(network.expect("checked"), &budget)
                        {
                            Ok(network) => network,
                            Err(error) => return Err(ready.reject_setup(error)),
                        };
                        Ok(ready
                            .with_network(network)
                            .execute(budget, run.timeout())?
                            .into_parts())
                    } else {
                        Ok(ready.execute(budget, run.timeout())?.into_parts())
                    }
                }
                Directive::Copy(copy) => {
                    self.copy_directive::<R>(storage, reaper, context, copy, artifacts)
                }
            })()
            .with_context(|| format!("directive {index}"))?;
        }
        Ok(CompletedDirectives { storage, reaper })
    }
    fn copy_directive<R: CommandStorage>(
        &self,
        storage: R::Unmounted,
        reaper: ChildReaper,
        context: &CapturedContext,
        directive: &Copy,
        artifacts: &crate::pipeline::artifacts::Artifacts<'_>,
    ) -> anyhow::Result<(R::Unmounted, ChildReaper)> {
        let budget = SetupBudget::begin();
        let mut mounted = R::mount(storage, self.mount_parent(), budget.deadline())?;
        let result = (|| -> anyhow::Result<()> {
            budget.check()?;
            if let Some(source) = artifacts
                .iter()
                .find_map(|(name, file)| (name.as_ref() == directive.artifact()).then_some(file))
            {
                mounted.copy_artifact(directive, source)?;
            } else {
                mounted.copy(context, directive)?;
            }
            budget.check()?;
            Ok(())
        })();
        let deadline = Instant::now() + TEARDOWN_TIMEOUT;
        let cleanup = mounted.unmount_until(deadline).map_err(anyhow::Error::new);
        let storage = match (result, cleanup) {
            (Ok(()), Ok(storage)) => storage,
            (Err(error), Ok(storage)) => {
                drop(storage);
                return Err(error);
            }
            (Ok(()), Err(error)) => return Err(error),
            (Err(error), Err(cleanup)) => {
                return Err(error.context(format!("COPY cleanup: {cleanup:#}")));
            }
        };
        Deadline::check(deadline)?;
        R::validate(&storage)?;
        Deadline::check(deadline)?;
        Ok((storage, reaper))
    }
}
