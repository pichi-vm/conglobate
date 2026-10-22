// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::LabelScope;
use libbpf_rs::{MapCore, MapFlags, MapType, Object, ObjectBuilder, OpenObject, ProgramType};

pub(super) mod private {
    pub trait Sealed {}
}
pub trait PolicyState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub struct Prepared;
#[derive(Debug)]
pub struct LoadedDisabled;
impl private::Sealed for Prepared {}
impl private::Sealed for LoadedDisabled {}
impl PolicyState for Prepared {
    type Resources = OpenObject;
}
impl PolicyState for LoadedDisabled {
    type Resources = Object;
}

/// Owned policy object. LoadedDisabled creates no hook attachments or pins;
/// PinnedDisabled retains the complete disabled hook set. Neither authorizes
/// commands. Activation is unavailable until anchored recovery is integrated.
#[derive(Debug)]
pub struct LabelPolicy<S: PolicyState> {
    pub(super) resources: S::Resources,
    pub(super) scope: LabelScope,
}
#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("label policy filesystem: {0}")]
    Io(#[from] std::io::Error),
    #[error("label policy object: {0}")]
    Object(#[from] libbpf_rs::Error),
    #[error("embedded label policy ABI mismatch: {0}")]
    Abi(&'static str),
    #[error("newly loaded label policy config is not disabled")]
    Enabled,
}

impl LabelPolicy<Prepared> {
    /// Parse the build-embedded trusted object without loading it into the kernel.
    /// There is no recipe-supplied object path or runtime download.
    pub fn new(scope: LabelScope) -> Result<Self, PolicyError> {
        let object = ObjectBuilder::default()
            .open_memory(include_bytes!(concat!(env!("OUT_DIR"), "/labels.bpf.o")))?;
        for (name, kind, key, value, count) in [
            ("config", MapType::Array, 4, 8, 1),
            ("targets", MapType::Hash, 8, 4, 2),
        ] {
            let map = object
                .maps()
                .find(|m| m.name() == name)
                .ok_or(PolicyError::Abi(name))?;
            if (
                map.map_type(),
                map.key_size(),
                map.value_size(),
                map.max_entries(),
            ) != (kind, key, value, count)
            {
                return Err(PolicyError::Abi(name));
            }
        }
        if object.progs().count() != 3 {
            return Err(PolicyError::Abi("program count"));
        }
        for (name, section) in [
            ("check_set", "lsm/inode_setxattr"),
            ("check_remove", "lsm/inode_removexattr"),
            ("skip_label_cap", "lsm/inode_xattr_skipcap"),
        ] {
            let program = object
                .progs()
                .find(|p| p.name() == name)
                .ok_or(PolicyError::Abi(name))?;
            if program.section() != section || program.prog_type() != ProgramType::Lsm {
                return Err(PolicyError::Abi(name));
            }
        }
        Ok(Self {
            resources: object,
            scope,
        })
    }

    /// Load verified programs and populate targets while config stays disabled.
    /// Any failure drops all unpinned, unattached kernel resources. This performs
    /// privileged kernel operations and must initially be qualified in a VM.
    pub fn load(self) -> Result<LabelPolicy<LoadedDisabled>, PolicyError> {
        let object = self.resources.load()?;
        {
            let config = object
                .maps()
                .find(|m| m.name() == "config")
                .ok_or(PolicyError::Abi("config"))?;
            if config
                .lookup(&0u32.to_ne_bytes(), MapFlags::ANY)?
                .as_deref()
                != Some(&[0u8; 8])
            {
                return Err(PolicyError::Enabled);
            }
            let targets = object
                .maps()
                .find(|m| m.name() == "targets")
                .ok_or(PolicyError::Abi("targets"))?;
            for target in self.scope.targets() {
                targets.update(&target.map_key(), &target.map_value(), MapFlags::NO_EXIST)?;
            }
        }
        Ok(LabelPolicy {
            resources: object,
            scope: self.scope,
        })
    }
}
impl<S: PolicyState> LabelPolicy<S> {
    pub fn scope(&self) -> &LabelScope {
        &self.scope
    }
}
