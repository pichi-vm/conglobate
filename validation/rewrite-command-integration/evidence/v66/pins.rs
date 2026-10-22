// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{LabelPolicy, LoadedDisabled, PolicyError, PolicyState, object::private};
use libbpf_rs::{Link, MapCore, Object};
use std::{
    fs::{self, File},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

/// Private bpffs directory created in the deployment init's mount namespace.
/// The trusted caller supplies a bpffs mounted there, keeps it attached until VM
/// disposal, and excludes out-of-protocol mutation. An inherited descriptor path
/// to another namespace's private bpffs does not satisfy that contract.
#[derive(Debug)]
pub struct PinDirectory {
    root: File,
    path: PathBuf,
    _parent: File,
    namespace: crate::anchor::InitMountNamespace,
}
impl PinDirectory {
    pub fn create(parent: &Path) -> io::Result<Self> {
        let namespace = crate::anchor::InitMountNamespace::capture()?;
        let parent = File::open(parent)?;
        let metadata = parent.metadata()?;
        if !metadata.is_dir()
            || metadata.uid() != 0
            || metadata.mode() & 0o022 != 0
            || rustix::fs::fstatfs(&parent)?.f_type as u64 != 0xcafe_4a11
        {
            return Err(io::Error::other(
                "policy pin parent must be a protected root-owned bpffs directory",
            ));
        }
        let directory = tempfile::Builder::new()
            .prefix("conglobate-policy-")
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir_in(format!("/proc/self/fd/{}", parent.as_raw_fd()))?;
        let root = File::open(directory.path())?;
        let path = directory.keep();
        Ok(Self {
            root,
            path,
            _parent: parent,
            namespace,
        })
    }
    fn entry(&self, name: &str) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}/{}", self.root.as_raw_fd(), name))
    }
    fn remove(&self) -> io::Result<()> {
        match fs::remove_dir(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }
}
impl Drop for PinDirectory {
    fn drop(&mut self) {
        if let Err(error) = self.remove() {
            eprintln!("policy pin directory retained for cleanup: {error}");
        }
    }
}

#[derive(Debug)]
pub struct PinnedDisabled;
impl private::Sealed for PinnedDisabled {}
impl PolicyState for PinnedDisabled {
    type Resources = DisabledPinRecovery;
}

/// Owns fully or partly installed DISABLED policy resources. Never use this
/// destructor policy for enabled links: activation needs a different owner.
#[derive(Debug)]
pub struct DisabledPinRecovery {
    links: Vec<Link>,
    object: Object,
    directory: PinDirectory,
}
#[derive(Debug, thiserror::Error)]
#[error("disabled policy pinning: {source}")]
pub struct PinFailure {
    source: PolicyError,
    recovery: DisabledPinRecovery,
}
impl PinFailure {
    pub fn into_parts(self) -> (PolicyError, DisabledPinRecovery) {
        (self.source, self.recovery)
    }
}
#[derive(Debug, thiserror::Error)]
#[error("disabled policy cleanup: {source}")]
pub struct PinCleanupFailure {
    source: io::Error,
    recovery: DisabledPinRecovery,
}
impl PinCleanupFailure {
    pub fn into_parts(self) -> (io::Error, DisabledPinRecovery) {
        (self.source, self.recovery)
    }
}
impl LabelPolicy<LoadedDisabled> {
    /// Attach/pin both target guards before skipcap, with config STILL zero.
    /// Failure retains partial resources for explicit cleanup. No commands may
    /// be admitted, and this directory alone is not a durable recovery anchor.
    pub fn pin_disabled(
        self,
        directory: PinDirectory,
    ) -> Result<LabelPolicy<PinnedDisabled>, PinFailure> {
        let mut recovery = DisabledPinRecovery {
            links: Vec::with_capacity(3),
            object: self.resources,
            directory,
        };
        if let Err(source) = recovery.install() {
            return Err(PinFailure { source, recovery });
        }
        Ok(LabelPolicy {
            resources: recovery,
            scope: self.scope,
        })
    }
}
impl LabelPolicy<PinnedDisabled> {
    pub fn close(self) -> Result<(), PinCleanupFailure> {
        self.resources.cleanup()
    }
}
impl DisabledPinRecovery {
    pub(super) fn config_handle(&self) -> Result<libbpf_rs::MapHandle, PolicyError> {
        self.directory.namespace.verify_current()?;
        let map = self
            .object
            .maps()
            .find(|map| map.name() == "config")
            .ok_or(PolicyError::Abi("config"))?;
        Ok(libbpf_rs::MapHandle::try_from(&map)?)
    }
    fn install(&mut self) -> Result<(), PolicyError> {
        for name in ["config", "targets"] {
            let mut map = self
                .object
                .maps_mut()
                .find(|m| m.name() == name)
                .ok_or(PolicyError::Abi(name))?;
            map.pin(self.directory.entry(name))?;
        }
        for name in ["check_set", "check_remove", "skip_label_cap"] {
            let program = self
                .object
                .progs_mut()
                .find(|p| p.name() == name)
                .ok_or(PolicyError::Abi(name))?;
            let link = program.attach_lsm()?;
            self.links.push(link);
            self.links
                .last_mut()
                .expect("newly owned link")
                .pin(self.directory.entry(name))?;
        }
        Ok(())
    }
    fn remove_pins(&self) -> io::Result<()> {
        // Config is permanently disabled in this owner. Remove exception first.
        for name in [
            "skip_label_cap",
            "check_remove",
            "check_set",
            "targets",
            "config",
        ] {
            match fs::remove_file(self.directory.entry(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        self.directory.remove()
    }
    pub fn cleanup(self) -> Result<(), PinCleanupFailure> {
        if let Err(source) = self.remove_pins() {
            return Err(PinCleanupFailure {
                source,
                recovery: self,
            });
        }
        Ok(())
    }
}
impl Drop for DisabledPinRecovery {
    fn drop(&mut self) {
        if let Err(error) = self.remove_pins() {
            eprintln!("disabled policy pins retained for recovery: {error}");
        }
    }
}
