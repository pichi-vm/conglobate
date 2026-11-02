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
    pub(crate) sources: &'a crate::pipeline::sources::Sources<'a, 'w>,
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
            sources,
            network,
        } = inputs;
        use anyhow::Context;
        anyhow::ensure!(
            network.is_some()
                || !directives
                    .iter()
                    .any(|d| matches!(d, Directive::Exec(run) if run.network)),
            "network-enabled directives require captured networking"
        );
        for (index, directive) in directives.iter().enumerate() {
            (storage, reaper) = (|| match directive {
                Directive::Exec(run) => {
                    let budget = SetupBudget::begin();
                    let receiver = self.receiver(run.exec.clone(), &budget)?;
                    let prepared = self.prepare_storage::<R>(storage, reaper, &budget)?;
                    let ready = prepared.launch(receiver, budget.deadline())?;
                    if run.network {
                        let network = match self.prepare_network(network.expect("checked"), &budget)
                        {
                            Ok(network) => network,
                            Err(error) => return Err(ready.reject_setup(error)),
                        };
                        Ok(ready
                            .with_network(network)
                            .execute(budget, run.timeout)?
                            .into_parts())
                    } else {
                        Ok(ready.execute(budget, run.timeout)?.into_parts())
                    }
                }
                Directive::Copy(copy) => {
                    self.copy_directive::<R>(storage, reaper, context, copy, sources)
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
        sources: &crate::pipeline::sources::Sources<'_, '_>,
    ) -> anyhow::Result<(R::Unmounted, ChildReaper)> {
        let budget = SetupBudget::begin();
        let mut mounted = R::mount(storage, self.mount_parent(), budget.deadline())?;
        let result = (|| -> anyhow::Result<()> {
            budget.check()?;
            if directive.from().is_some() {
                sources.copy(&mut mounted, directive, &budget)?;
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
