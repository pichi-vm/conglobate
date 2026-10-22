// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Immutable descriptions for the labels.bpf.c map ABI, not authorization.
//! Numeric identities are reusable: a future activation owner must retain
//! independent namespace/mount anchors and command containment through teardown.
use std::{collections::BTreeSet, num::NonZeroU32};

mod object;
pub use object::{LabelPolicy, LoadedDisabled, PolicyError, PolicyState, Prepared};
mod pins;
pub use pins::{DisabledPinRecovery, PinCleanupFailure, PinDirectory, PinFailure, PinnedDisabled};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct NamespaceId(NonZeroU32);
impl TryFrom<u64> for NamespaceId {
    type Error = ScopeError;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        u32::try_from(value)
            .ok()
            .and_then(NonZeroU32::new)
            .map(Self)
            .ok_or(ScopeError::Namespace)
    }
}

/// An exact superblock owner/device pair. Neither field is a wildcard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FilesystemTarget {
    owner: NamespaceId,
    device: u32,
}
impl FilesystemTarget {
    /// Translate stat's userspace dev_t to the kernel's 12:20-bit s_dev.
    pub fn new(owner_namespace: u64, device: u64) -> Result<Self, ScopeError> {
        let major = rustix::fs::major(device);
        let minor = rustix::fs::minor(device);
        if major >= (1 << 12) || minor >= (1 << 20) {
            return Err(ScopeError::Device);
        }
        Ok(Self {
            owner: owner_namespace.try_into()?,
            device: (major << 20) | minor,
        })
    }

    /// Native-endian kernel map ABI; this is not the CBOR inventory format.
    pub fn map_key(&self) -> [u8; 8] {
        let mut bytes = [0; 8];
        bytes[..4].copy_from_slice(&self.owner.0.get().to_ne_bytes());
        bytes[4..].copy_from_slice(&self.device.to_ne_bytes());
        bytes
    }
    pub fn map_value(&self) -> [u8; 4] {
        1u32.to_ne_bytes()
    }
}

/// Describes one command subject and its one or two assigned writable mounts.
/// Constructing this value neither pins identities nor installs/enables policy.
#[derive(Debug, Clone)]
pub struct LabelScope {
    namespace: NamespaceId,
    root_uid: NonZeroU32,
    targets: BTreeSet<FilesystemTarget>,
}
#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("namespace identity must be a nonzero 32-bit inode number")]
    Namespace,
    #[error("device identity exceeds kernel s_dev representation")]
    Device,
    #[error("mapped command root must have a valid nonzero outer UID")]
    Root,
    #[error("label policy requires one or two distinct assigned filesystems")]
    TargetCount,
    #[error("duplicate assigned filesystem")]
    Duplicate,
    #[error("superblock must be owned outside the command user namespace")]
    SuperblockOwner,
}
impl LabelScope {
    pub fn new(
        command_namespace: u64,
        mapped_root_uid: u32,
        filesystems: impl IntoIterator<Item = FilesystemTarget>,
    ) -> Result<Self, ScopeError> {
        let namespace = command_namespace.try_into()?;
        if mapped_root_uid == u32::MAX {
            return Err(ScopeError::Root);
        }
        let root_uid = NonZeroU32::new(mapped_root_uid).ok_or(ScopeError::Root)?;
        let mut targets = BTreeSet::new();
        for target in filesystems {
            if target.owner == namespace {
                return Err(ScopeError::SuperblockOwner);
            }
            if !targets.insert(target) {
                return Err(ScopeError::Duplicate);
            }
            if targets.len() > 2 {
                return Err(ScopeError::TargetCount);
            }
        }
        if targets.is_empty() {
            return Err(ScopeError::TargetCount);
        }
        Ok(Self {
            namespace,
            root_uid,
            targets,
        })
    }
    pub fn targets(&self) -> impl ExactSizeIterator<Item = &FilesystemTarget> {
        self.targets.iter()
    }
    /// Config map value. The loader must write it LAST, after all guards and
    /// target entries are pinned, and before command admission. Zero disables.
    pub fn subject_value(&self) -> [u8; 8] {
        let mut bytes = [0; 8];
        bytes[..4].copy_from_slice(&self.namespace.0.get().to_ne_bytes());
        bytes[4..].copy_from_slice(&self.root_uid.get().to_ne_bytes());
        bytes
    }
}
