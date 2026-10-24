// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Ordered recipe directives; failures never yield an accepted stage receipt.
use super::{
    CommandSandbox, ExecutionStorage, SetupBudget, StorageExecutionFailure,
    StoragePreparationFailure,
    teardown::{Deadline, TEARDOWN_TIMEOUT},
};
use crate::{
    context::CapturedContext,
    copy::CopyError,
    process::ChildReaper,
    recipe::{Copy, Directive},
    storage::{BootstrapVolumes, FailureDiagnostics, Mounted, Volume},
};
use std::{io, time::Instant};

/// Sealed layouts supporting trusted COPY between finalized commands.
pub(crate) trait DirectiveStorage:
    ExecutionStorage + super::PreparationStorage + super::NetworkEligibleStorage
{
    #[doc(hidden)]
    fn copy(&mut self, context: &CapturedContext, directive: &Copy) -> Result<(), CopyError>;
}
impl DirectiveStorage for Volume<'_, '_, '_, Mounted> {
    fn copy(&mut self, context: &CapturedContext, directive: &Copy) -> Result<(), CopyError> {
        self.copy(context, directive)
    }
}
impl DirectiveStorage for BootstrapVolumes<'_, '_, '_, Mounted> {
    fn copy(&mut self, context: &CapturedContext, directive: &Copy) -> Result<(), CopyError> {
        self.copy(context, directive)
    }
}
#[derive(Debug)]
pub(crate) struct CompletedDirectives<R: DirectiveStorage> {
    storage: R::Unmounted,
    reaper: ChildReaper,
}
impl<R: DirectiveStorage> CompletedDirectives<R> {
    pub(crate) fn into_parts(self) -> (R::Unmounted, ChildReaper) {
        (self.storage, self.reaper)
    }
}
#[derive(Debug, thiserror::Error)]
pub(crate) enum StageFailure<'l, R: DirectiveStorage> {
    #[error("directive {index}: {source}")]
    Before {
        index: usize,
        source: io::Error,
        storage: R::Unmounted,
        reaper: ChildReaper,
    },
    #[error("directive {index}: {failure}")]
    Preparation {
        index: usize,
        failure: Box<StoragePreparationFailure<R>>,
    },
    #[error("directive {index}: {failure}")]
    Execution {
        index: usize,
        failure: Box<StorageExecutionFailure<'l, R>>,
    },
    #[error("directive {index}: {failure}")]
    NetworkExecution {
        index: usize,
        failure: Box<StorageExecutionFailure<'l, super::NetworkedStorage<R>>>,
    },
    #[error("directive {index}: {failure}")]
    Copy {
        index: usize,
        failure: Box<CopyStageFailure<R>>,
    },
}
#[derive(Debug)]
enum CopyCleanup<R: DirectiveStorage> {
    Mount(R::MountFailure),
    Unmount(R::UnmountFailure),
    Complete(R::Unmounted),
}
#[derive(Debug, thiserror::Error)]
#[error("COPY: {diagnostics}")]
pub(crate) struct CopyStageFailure<R: DirectiveStorage> {
    diagnostics: FailureDiagnostics,
    cleanup: CopyCleanup<R>,
    deadline: Instant,
    reaper: ChildReaper,
}
impl<R: DirectiveStorage> CopyStageFailure<R> {
    pub(crate) fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    /// Retry only cleanup under its original cutoff. The failed COPY remains failed.
    pub(crate) fn retry_cleanup(mut self) -> Self {
        if matches!(self.cleanup, CopyCleanup::Complete(_)) {
            return self;
        }
        if let Err(error) = Deadline::check(self.deadline) {
            self.diagnostics = self.diagnostics.followed_by(error.into());
            return self;
        }
        let result = match self.cleanup {
            CopyCleanup::Mount(failure) => R::rollback_mount(failure, self.deadline),
            CopyCleanup::Unmount(failure) => R::retry_unmount(failure),
            CopyCleanup::Complete(_) => unreachable!(),
        };
        self.cleanup = match result {
            Ok(storage) => CopyCleanup::Complete(storage),
            Err(failure) => {
                self.diagnostics = self
                    .diagnostics
                    .followed_by(R::unmount_diagnostics(&failure).latest_only());
                CopyCleanup::Unmount(failure)
            }
        };
        if let Err(error) = Deadline::check(self.deadline) {
            self.diagnostics = self.diagnostics.followed_by(error.into());
        }
        self
    }
    pub(crate) fn into_rejected_storage(
        self,
    ) -> Result<(FailureDiagnostics, R::Unmounted, ChildReaper), Box<Self>> {
        match self.cleanup {
            CopyCleanup::Complete(storage) => Ok((self.diagnostics, storage, self.reaper)),
            _ => Err(Box::new(self)),
        }
    }
}
impl<'l> CommandSandbox<'l> {
    /// Offline execution entrypoint; enabled steps require captured networking.
    pub(crate) fn directives<R: DirectiveStorage>(
        &self,
        storage: R::Unmounted,
        reaper: ChildReaper,
        context: &CapturedContext,
        directives: &[Directive],
        carapace: Option<&[u8; 32]>,
    ) -> Result<CompletedDirectives<R>, Box<StageFailure<'l, R>>> {
        self.directives_with_network::<R>(storage, reaper, context, directives, carapace, None)
    }
    /// Execute in authored order. Each RUN gets fresh mounts and its recipe
    /// timeout; each COPY runs exclusively between quiescent commands.
    /// Network-enabled steps require the captured build environment.
    pub(crate) fn directives_with_network<R: DirectiveStorage>(
        &self,
        mut storage: R::Unmounted,
        mut reaper: ChildReaper,
        context: &CapturedContext,
        directives: &[Directive],
        carapace: Option<&[u8; 32]>,
        network: Option<&crate::network::CommandNetworking<'_>>,
    ) -> Result<CompletedDirectives<R>, Box<StageFailure<'l, R>>> {
        if network.is_none()
            && let Some(index) = directives
                .iter()
                .position(|d| matches!(d, Directive::Run(run) if run.network()))
        {
            return Err(Box::new(StageFailure::Before {
                index,
                source: io::Error::other("network-enabled directives require captured networking"),
                storage,
                reaper,
            }));
        }
        for (index, directive) in directives.iter().enumerate() {
            match directive {
                Directive::Run(run) => {
                    let budget = SetupBudget::begin();
                    let script = match carapace {
                        Some(root) => {
                            format!("export CARAPACE={}\n{}", hex::encode(root), run.command())
                        }
                        None => run.command().to_owned(),
                    };
                    let receiver = match self.receiver(script, &budget) {
                        Ok(receiver) => receiver,
                        Err(source) => {
                            return Err(Box::new(StageFailure::Before {
                                index,
                                source,
                                storage,
                                reaper,
                            }));
                        }
                    };
                    let prepared = self
                        .prepare_storage::<R>(storage, reaper, &budget)
                        .map_err(|failure| {
                            Box::new(StageFailure::Preparation { index, failure })
                        })?;
                    let ready =
                        prepared
                            .launch(receiver, budget.deadline())
                            .map_err(|failure| {
                                Box::new(StageFailure::Preparation { index, failure })
                            })?;
                    (storage, reaper) = if run.network() {
                        let networking = network.expect("preflight checked captured networking");
                        let network = match self.prepare_network(networking, &budget) {
                            Ok(network) => network,
                            Err(source) => {
                                return Err(Box::new(StageFailure::Execution {
                                    index,
                                    failure: ready.reject_setup(source),
                                }));
                            }
                        };
                        ready
                            .with_network(network)
                            .execute(budget, run.timeout())
                            .map_err(|failure| {
                                Box::new(StageFailure::NetworkExecution { index, failure })
                            })?
                            .into_parts()
                    } else {
                        ready
                            .execute(budget, run.timeout())
                            .map_err(|failure| {
                                Box::new(StageFailure::Execution { index, failure })
                            })?
                            .into_parts()
                    };
                }
                Directive::Copy(copy) => {
                    (storage, reaper) = self
                        .copy_directive::<R>(storage, reaper, context, copy)
                        .map_err(|failure| Box::new(StageFailure::Copy { index, failure }))?;
                }
            }
        }
        Ok(CompletedDirectives { storage, reaper })
    }
    fn copy_directive<R: DirectiveStorage>(
        &self,
        storage: R::Unmounted,
        reaper: ChildReaper,
        context: &CapturedContext,
        directive: &Copy,
    ) -> Result<(R::Unmounted, ChildReaper), Box<CopyStageFailure<R>>> {
        let budget = SetupBudget::begin();
        let mut mounted = match R::mount(storage, self.mount_parent(), budget.deadline()) {
            Ok(mounted) => mounted,
            Err((diagnostics, failure)) => {
                return Err(Box::new(
                    CopyStageFailure::<R> {
                        diagnostics,
                        cleanup: CopyCleanup::Mount(failure),
                        deadline: Instant::now() + TEARDOWN_TIMEOUT,
                        reaper,
                    }
                    .retry_cleanup(),
                ));
            }
        };
        let result = budget
            .check()
            .and_then(|()| mounted.copy(context, directive).map_err(io::Error::other))
            .and_then(|()| budget.check());
        let deadline = Instant::now() + TEARDOWN_TIMEOUT;
        let storage = match mounted.unmount_until(deadline) {
            Ok(storage) => storage,
            Err(failure) => {
                let cleanup_error = R::unmount_diagnostics(&failure).latest_only();
                let diagnostics = match result {
                    Ok(()) => cleanup_error,
                    Err(source) => FailureDiagnostics::from(source).followed_by(cleanup_error),
                };
                return Err(Box::new(CopyStageFailure {
                    diagnostics,
                    cleanup: CopyCleanup::Unmount(failure),
                    deadline,
                    reaper,
                }));
            }
        };
        let result = result
            .and_then(|()| Deadline::check(deadline))
            .and_then(|()| R::validate(&storage))
            .and_then(|()| Deadline::check(deadline));
        match result {
            Ok(()) => Ok((storage, reaper)),
            Err(source) => Err(Box::new(CopyStageFailure {
                diagnostics: source.into(),
                cleanup: CopyCleanup::Complete(storage),
                deadline,
                reaper,
            })),
        }
    }
}
