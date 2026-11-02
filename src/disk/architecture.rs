// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// Target root types defined by the Discoverable Partitions Specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Architecture {
    /// 64-bit x86 Linux root.
    X86_64,

    /// 64-bit ARM Linux root.
    Aarch64,
}

impl Architecture {
    pub(crate) fn current() -> std::io::Result<Self> {
        match std::env::consts::ARCH {
            "x86_64" => Ok(Self::X86_64),
            "aarch64" => Ok(Self::Aarch64),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "unsupported root architecture",
            )),
        }
    }

    pub(super) fn partition_type(self) -> gpt::partition_types::Type {
        match self {
            Self::X86_64 => gpt::partition_types::LINUX_ROOT_X64,
            Self::Aarch64 => gpt::partition_types::LINUX_ROOT_ARM_64,
        }
    }
}
