// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Ordered recipe directives; failures never yield an accepted stage receipt.
use super::{
    CommandSandbox, CommandStorage, SetupBudget,
    teardown::{Deadline, TEARDOWN_TIMEOUT},
};
use crate::{
    context::CapturedContext,
    process::ChildReaper,
    recipe::{Copy, Directive},
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
impl<'l> CommandSandbox<'l> {
    pub(crate) fn directives_with_network<R: CommandStorage>(
        &self,
        mut storage: R::Unmounted,
        mut reaper: ChildReaper,
        context: &CapturedContext,
        directives: &[Directive],
        carapace: Option<&[u8; 32]>,
        network: Option<&crate::network::CommandNetworking<'_>>,
    ) -> anyhow::Result<CompletedDirectives<R>> {
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
                Directive::Copy(copy) => self.copy_directive::<R>(storage, reaper, context, copy),
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
    ) -> anyhow::Result<(R::Unmounted, ChildReaper)> {
        let budget = SetupBudget::begin();
        let mut mounted = R::mount(storage, self.mount_parent(), budget.deadline())?;
        let result = (|| -> anyhow::Result<()> {
            budget.check()?;
            mounted.copy(context, directive)?;
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
