// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::value::FilesystemSize;
use std::{ffi::OsString, io, path::Path};

/// FAT32 on a bounded 4 KiB-sector ESP, with 4 KiB clusters.
#[derive(Debug)]
pub(crate) struct Fat32Format(FilesystemSize);

impl TryFrom<FilesystemSize> for Fat32Format {
    type Error = io::Error;
    fn try_from(size: FilesystemSize) -> io::Result<Self> {
        // Reserve room for both FATs and at least 65,525 data clusters. The
        // upper bound keeps one-sector clusters within FAT32's 28-bit indices.
        if !(257 * 1024 * 1024..=1024 * 1024 * 1024 * 1024).contains(&size.bytes()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ESP must be between 257 MiB and 1 TiB for 4 KiB-sector FAT32",
            ));
        }
        Ok(Self(size))
    }
}

impl Fat32Format {
    pub(crate) fn bytes(&self) -> u64 {
        self.0.bytes()
    }

    pub(crate) fn arguments(&self, device: &Path) -> Vec<OsString> {
        // -I permits the supervisor's bounded dm-linear view, which has no
        // kernel partition-device identity. It never receives an entire disk.
        let mut arguments = ["-I", "-F", "32", "-S", "4096", "-s", "1", "-n", "ESP"]
            .map(OsString::from)
            .to_vec();
        arguments.push(device.as_os_str().to_owned());
        arguments
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_capacities_outside_the_fat32_cluster_range() {
        for size in ["16MiB", "256MiB", "1025GiB"] {
            assert!(Fat32Format::try_from(size.parse::<FilesystemSize>().unwrap()).is_err());
        }
        for size in ["257MiB", "512MiB", "1024GiB"] {
            assert!(Fat32Format::try_from(size.parse::<FilesystemSize>().unwrap()).is_ok());
        }
    }
}
