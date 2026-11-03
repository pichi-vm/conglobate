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
            assert!(
                Fat32Format::try_from(
                    serde_json::from_value::<FilesystemSize>(serde_json::json!(size)).unwrap()
                )
                .is_err()
            );
        }
        for size in ["257MiB", "512MiB", "1024GiB"] {
            assert!(
                Fat32Format::try_from(
                    serde_json::from_value::<FilesystemSize>(serde_json::json!(size)).unwrap()
                )
                .is_ok()
            );
        }
    }
    #[test]
    #[ignore = "requires the mkfs.fat executable"]
    fn formats_a_four_kib_sector_fat32_filesystem() {
        let format = Fat32Format::try_from(
            serde_json::from_value::<FilesystemSize>(serde_json::json!("257MiB")).unwrap(),
        )
        .unwrap();
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("esp.fat");
        std::fs::File::create(&path)
            .unwrap()
            .set_len(format.bytes())
            .unwrap();
        assert!(
            std::process::Command::new("mkfs.fat")
                .args(format.arguments(&path))
                .status()
                .unwrap()
                .success()
        );
        let mut file = std::fs::File::open(path).unwrap();
        let mut sector = [0; 512];
        std::io::Read::read_exact(&mut file, &mut sector).unwrap();
        assert_eq!(u16::from_le_bytes([sector[11], sector[12]]), 4096);
        assert_eq!(sector[13], 1);
        assert_eq!(&sector[82..90], b"FAT32   ");
    }
}
