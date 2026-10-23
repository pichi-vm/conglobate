// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use rustix::thread::{self, CapabilitySet, CapabilitySets};
use std::{fs, io};

/// Immutable command capability policy. This candidate filesystem-build profile
/// still requires distro qualification together with the syscall/LSM policies.
/// It is not sandbox admission authority and cannot establish namespace isolation.
#[derive(Debug)]
pub struct CommandPrivileges {
    allowed: CapabilitySet,
}

/// Privilege changes are irreversible and can be partially applied. The trusted
/// helper must exit on failure; retrying command execution is not permitted.
#[derive(Debug, thiserror::Error)]
#[error("command privilege finalization failed; helper must exit: {0}")]
pub struct PrivilegeFailure(#[from] io::Error);

impl CommandPrivileges {
    pub fn filesystem() -> Self {
        Self {
            allowed: CapabilitySet::CHOWN
                | CapabilitySet::DAC_OVERRIDE
                | CapabilitySet::FOWNER
                | CapabilitySet::FSETID
                | CapabilitySet::KILL
                | CapabilitySet::SETGID
                | CapabilitySet::SETUID
                | CapabilitySet::SYS_CHROOT
                | CapabilitySet::SETFCAP
                | CapabilitySet::NET_BIND_SERVICE,
        }
    }

    pub fn allowed(&self) -> CapabilitySet {
        self.allowed
    }

    /// Apply to the current single-threaded trusted setup process, after its
    /// namespace/mount work and before executing any image-provided code.
    /// The caller must prevent concurrent thread creation and must immediately
    /// abort the helper on error. No descriptor, syscall or LSM isolation is
    /// established here. Do not call from the build supervisor.
    pub fn apply(self) -> Result<(), PrivilegeFailure> {
        self.finalize().map_err(PrivilegeFailure)
    }

    fn finalize(self) -> io::Result<()> {
        if fs::read_dir("/proc/self/task")?
            .collect::<Result<Vec<_>, _>>()?
            .len()
            != 1
        {
            return Err(io::Error::other("privilege setup requires one thread"));
        }
        // Query the kernel, not cap_last_cap from a possibly image-owned procfs.
        // Probe the entire v3 capability ABI; unknown supported bits are denied.
        let mut supported = CapabilitySet::empty();
        for bit in 0..64 {
            let cap = CapabilitySet::from_bits_retain(1 << bit);
            match thread::capability_is_in_bounding_set(cap) {
                Ok(present) => {
                    supported |= cap;
                    if self.allowed.contains(cap) && !present {
                        return Err(io::Error::other("required capability is not available"));
                    }
                }
                Err(rustix::io::Errno::INVAL) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if !supported.contains(self.allowed) {
            return Err(io::Error::other("kernel lacks required capabilities"));
        }
        thread::set_no_new_privs(true)?;
        thread::clear_ambient_capability_set()?;
        // Keep effective SETPCAP until all bounding-set drops have completed.
        for bit in 0..64 {
            let cap = CapabilitySet::from_bits_retain(1 << bit);
            if supported.contains(cap) && !self.allowed.contains(cap) {
                thread::remove_capability_from_bounding_set(cap)?;
            }
        }
        thread::set_capabilities(
            None,
            CapabilitySets {
                effective: self.allowed,
                permitted: self.allowed,
                inheritable: CapabilitySet::empty(),
            },
        )?;
        let actual = thread::capabilities(None)?;
        if actual.effective != self.allowed
            || actual.permitted != self.allowed
            || !actual.inheritable.is_empty()
            || !thread::no_new_privs()?
        {
            return Err(io::Error::other("command capability verification failed"));
        }
        for bit in 0..64 {
            let cap = CapabilitySet::from_bits_retain(1 << bit);
            if supported.contains(cap)
                && (thread::capability_is_in_bounding_set(cap)? != self.allowed.contains(cap)
                    || thread::capability_is_in_ambient_set(cap)?)
            {
                return Err(io::Error::other(
                    "command bounding/ambient verification failed",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filesystem_commands_have_no_sys_admin_authority() {
        // A design requirement independent of the chosen filesystem allowlist.
        // VM tests separately verify application to all five capability sets.
        assert!(
            !CommandPrivileges::filesystem()
                .allowed()
                .contains(CapabilitySet::SYS_ADMIN)
        );
    }
}
