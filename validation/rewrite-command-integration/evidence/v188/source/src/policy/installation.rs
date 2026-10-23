// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{
    BoundCleanup, BoundDisabled, DisabledPinRecovery, FilesystemTarget, LabelPolicy, PinDirectory,
    PinnedDisabled, PolicyIdentities,
};
use crate::{
    anchor::FilesystemPin,
    isolation::{PinnedNamespace, UserNamespace},
    storage::FailureDiagnostics,
};
use std::{
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Instant,
};

/// Identity-owning installation of the embedded policy for supervisor-created
/// superblocks. Construction grants no command authority.
#[derive(Debug)]
pub struct PolicyInstallation {
    identities: PolicyIdentities,
    parent: PathBuf,
}
#[derive(Debug)]
enum Pending {
    None,
    Directory(PinDirectory),
    Pinned(LabelPolicy<PinnedDisabled>),
    Partial(DisabledPinRecovery),
}
#[derive(Debug)]
enum Failed {
    Unbound {
        pending: Pending,
        identities: PolicyIdentities,
    },
    Bound(LabelPolicy<BoundDisabled>),
    BoundRetry(BoundCleanup),
}
#[derive(Debug, thiserror::Error)]
#[error("policy installation: {diagnostics}")]
pub struct InstallationFailure {
    diagnostics: FailureDiagnostics,
    failed: Failed,
}
impl PolicyInstallation {
    pub(crate) fn into_identities(self) -> PolicyIdentities {
        self.identities
    }

    pub fn new(
        namespace: UserNamespace<PinnedNamespace>,
        filesystems: Vec<FilesystemPin>,
        parent: &Path,
    ) -> Self {
        Self {
            identities: PolicyIdentities::new(namespace, filesystems),
            parent: parent.to_owned(),
        }
    }
    fn check(deadline: Instant) -> io::Result<()> {
        if Instant::now() >= deadline {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "policy setup deadline expired",
            ))
        } else {
            Ok(())
        }
    }
    /// Every acquisition shares the caller's original setup deadline. The
    /// supervisor must have created the assigned superblocks in the initial
    /// user namespace; metadata alone cannot prove that superblock provenance.
    pub fn install(
        self,
        deadline: Instant,
    ) -> Result<LabelPolicy<BoundDisabled>, Box<InstallationFailure>> {
        let loaded = (|| {
            Self::check(deadline)?;
            let owner = fs::metadata("/proc/self/ns/user")?;
            let initial = fs::metadata("/proc/1/ns/user")?;
            if (owner.dev(), owner.ino()) != (initial.dev(), initial.ino()) {
                return Err(io::Error::other(
                    "policy installation requires initial user namespace",
                ));
            }
            let scope = self.identities.scope(owner.ino())?;
            Self::check(deadline)?;
            let prepared = LabelPolicy::new(scope).map_err(io::Error::other)?;
            Self::check(deadline)?;
            let loaded = prepared.load().map_err(io::Error::other)?;
            Self::check(deadline)?;
            let directory = PinDirectory::create(&self.parent)?;
            Ok((loaded, directory))
        })();
        let (loaded, directory) = match loaded {
            Ok(pair) => pair,
            Err(error) => {
                return Err(InstallationFailure::unbound(
                    error,
                    Pending::None,
                    self.identities,
                ));
            }
        };
        if let Err(error) = Self::check(deadline) {
            return Err(InstallationFailure::unbound(
                error,
                Pending::Directory(directory),
                self.identities,
            ));
        }
        let pinned = match loaded.pin_disabled(directory) {
            Ok(pinned) => pinned,
            Err(failure) => {
                let (error, partial) = failure.into_parts();
                return Err(InstallationFailure::unbound(
                    io::Error::other(error),
                    Pending::Partial(partial),
                    self.identities,
                ));
            }
        };
        if let Err(error) = Self::check(deadline) {
            return Err(InstallationFailure::unbound(
                error,
                Pending::Pinned(pinned),
                self.identities,
            ));
        }
        let (namespace, filesystems) = self.identities.into_parts();
        let bound = match pinned.bind(namespace, filesystems) {
            Ok(bound) => bound,
            Err(failure) => {
                let (error, pinned, identities) = failure.into_parts();
                return Err(InstallationFailure::unbound(
                    error,
                    Pending::Pinned(pinned),
                    identities,
                ));
            }
        };
        match Self::check(deadline) {
            Ok(()) => Ok(bound),
            Err(error) => Err(Box::new(InstallationFailure {
                diagnostics: error.into(),
                failed: Failed::Bound(bound),
            })),
        }
    }
}
impl InstallationFailure {
    fn unbound(error: io::Error, pending: Pending, identities: PolicyIdentities) -> Box<Self> {
        Box::new(Self {
            diagnostics: error.into(),
            failed: Failed::Unbound {
                pending,
                identities,
            },
        })
    }
    pub fn diagnostics(&self) -> &FailureDiagnostics {
        &self.diagnostics
    }
    /// Remove all disabled policy resources before releasing identity ownership.
    /// The enclosing preparation owner retains the cleanup deadline and storage.
    pub fn cleanup(self) -> Result<PolicyIdentities, Box<Self>> {
        let failed = match self.failed {
            Failed::Unbound {
                pending,
                identities,
            } => {
                let result = match pending {
                    Pending::None => return Ok(identities),
                    Pending::Directory(directory) => {
                        return match directory.close() {
                            Ok(()) => Ok(identities),
                            Err((error, directory)) => Err(Box::new(Self {
                                diagnostics: self.diagnostics.followed_by(error.into()),
                                failed: Failed::Unbound {
                                    pending: Pending::Directory(directory),
                                    identities,
                                },
                            })),
                        };
                    }
                    Pending::Pinned(policy) => policy.close(),
                    Pending::Partial(policy) => policy.cleanup(),
                };
                match result {
                    Ok(()) => return Ok(identities),
                    Err(failure) => {
                        let (error, pending) = failure.into_parts();
                        (
                            error,
                            Failed::Unbound {
                                pending: Pending::Partial(pending),
                                identities,
                            },
                        )
                    }
                }
            }
            Failed::Bound(policy) => match policy.close() {
                Ok(identities) => return Ok(identities),
                Err(failure) => {
                    let (error, policy) = failure.into_parts();
                    (error, Failed::BoundRetry(policy))
                }
            },
            Failed::BoundRetry(policy) => match policy.retry() {
                Ok(identities) => return Ok(identities),
                Err(failure) => {
                    let (error, policy) = failure.into_parts();
                    (error, Failed::BoundRetry(policy))
                }
            },
        };
        Err(Box::new(Self {
            diagnostics: self.diagnostics.followed_by(failed.0.into()),
            failed: failed.1,
        }))
    }
}
impl PolicyIdentities {
    pub(super) fn scope(&self, owner: u64) -> io::Result<super::LabelScope> {
        let targets = self
            .filesystems()
            .iter()
            .map(|pin| {
                FilesystemTarget::new(owner, pin.metadata()?.dev()).map_err(io::Error::other)
            })
            .collect::<io::Result<Vec<_>>>()?;
        self.namespace()
            .label_scope(targets)
            .map_err(io::Error::other)
    }
}
