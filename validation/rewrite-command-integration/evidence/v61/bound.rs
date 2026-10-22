// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{
    DisabledPinRecovery, LabelPolicy, LabelScope, PinnedDisabled, PolicyState, object::private,
};
use crate::{
    anchor::FilesystemPin,
    isolation::{PinnedNamespace, UserNamespace},
};
use std::{collections::BTreeSet, io, os::unix::fs::MetadataExt};

/// Disabled policy with its namespace and assigned superblock pins owned as a
/// unit. This does not permit activation or command admission.
///
/// ```compile_fail
/// use conglobate::policy::{BoundDisabled, LabelPolicy};
/// fn cannot_extract_live_identity_owners(policy: LabelPolicy<BoundDisabled>) {
///     policy.into_parts();
/// }
/// ```
#[derive(Debug)]
pub struct BoundDisabled;
impl private::Sealed for BoundDisabled {}
impl PolicyState for BoundDisabled {
    type Resources = BoundResources;
}
#[derive(Debug)]
pub struct BoundResources {
    policy: DisabledPinRecovery,
    identities: PolicyIdentities,
}
#[derive(Debug)]
pub struct PolicyIdentities {
    namespace: UserNamespace<PinnedNamespace>,
    filesystems: Vec<FilesystemPin>,
}
impl PolicyIdentities {
    fn verify(&self, scope: &LabelScope) -> io::Result<()> {
        let expected = self
            .namespace
            .label_scope(scope.targets().copied())
            .map_err(io::Error::other)?;
        if expected.subject_value() != scope.subject_value() {
            return Err(io::Error::other(
                "policy subject differs from owned namespace mapping",
            ));
        }
        let targets: BTreeSet<_> = scope.targets().map(|target| target.device).collect();
        if self.filesystems.len() != scope.targets().len()
            || targets.len() != self.filesystems.len()
        {
            return Err(io::Error::other(
                "policy requires one pin per distinct assigned filesystem",
            ));
        }
        let mut devices = BTreeSet::new();
        for filesystem in &self.filesystems {
            let device = filesystem.metadata()?.dev();
            let major = rustix::fs::major(device);
            let minor = rustix::fs::minor(device);
            if major >= (1 << 12) || minor >= (1 << 20) || !devices.insert((major << 20) | minor) {
                return Err(io::Error::other(
                    "invalid or duplicate assigned filesystem pin",
                ));
            }
        }
        if devices != targets {
            return Err(io::Error::other(
                "policy targets differ from owned filesystem pins",
            ));
        }
        Ok(())
    }
    /// Returned only before binding or after disabled policy cleanup succeeds.
    /// The caller still owns explicit pin release and any remaining storage work.
    pub fn into_parts(self) -> (UserNamespace<PinnedNamespace>, Vec<FilesystemPin>) {
        (self.namespace, self.filesystems)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("binding policy identities: {source}")]
pub struct BindFailure {
    source: io::Error,
    policy: LabelPolicy<PinnedDisabled>,
    identities: PolicyIdentities,
}
impl BindFailure {
    pub fn into_parts(self) -> (io::Error, LabelPolicy<PinnedDisabled>, PolicyIdentities) {
        (self.source, self.policy, self.identities)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("bound disabled policy cleanup: {source}")]
pub struct BoundCloseFailure {
    source: io::Error,
    cleanup: BoundCleanup,
}
impl BoundCloseFailure {
    pub fn into_parts(self) -> (io::Error, BoundCleanup) {
        (self.source, self.cleanup)
    }
}
impl LabelPolicy<PinnedDisabled> {
    /// Consume the matching identity owners before any future activation.
    /// Checks subject mapping and exact device membership. Superblock owner
    /// namespace provenance must come from trusted filesystem construction;
    /// stat metadata cannot independently prove that part of the description.
    pub fn bind(
        self,
        namespace: UserNamespace<PinnedNamespace>,
        filesystems: Vec<FilesystemPin>,
    ) -> Result<LabelPolicy<BoundDisabled>, Box<BindFailure>> {
        let identities = PolicyIdentities {
            namespace,
            filesystems,
        };
        if let Err(source) = identities.verify(&self.scope) {
            return Err(Box::new(BindFailure {
                source,
                policy: self,
                identities,
            }));
        }
        Ok(LabelPolicy {
            scope: self.scope,
            resources: BoundResources {
                policy: self.resources,
                identities,
            },
        })
    }
}
/// Cleanup has started; this owner cannot be rebound or activated.
///
/// ```compile_fail
/// use conglobate::policy::{BoundCleanup, BoundDisabled, LabelPolicy};
/// fn cannot_recover_ready_policy(cleanup: BoundCleanup) -> LabelPolicy<BoundDisabled> {
///     cleanup
/// }
/// ```
#[derive(Debug)]
pub struct BoundCleanup {
    resources: BoundResources,
}
impl LabelPolicy<BoundDisabled> {
    /// Return identity owners only after all disabled policy resources have been
    /// removed. Failure retains cleanup-only ownership of the entire aggregate.
    pub fn close(self) -> Result<PolicyIdentities, Box<BoundCloseFailure>> {
        BoundCleanup {
            resources: self.resources,
        }
        .retry()
    }
}
impl BoundCleanup {
    pub fn retry(self) -> Result<PolicyIdentities, Box<BoundCloseFailure>> {
        let BoundResources { policy, identities } = self.resources;
        match policy.cleanup() {
            Ok(()) => Ok(identities),
            Err(failure) => {
                let (source, policy) = failure.into_parts();
                Err(Box::new(BoundCloseFailure {
                    source,
                    cleanup: Self {
                        resources: BoundResources { policy, identities },
                    },
                }))
            }
        }
    }
}
