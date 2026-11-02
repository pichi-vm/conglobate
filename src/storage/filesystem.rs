// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{fs::File, io, os::unix::fs::FileExt};

/// Select the kernel driver; the driver validates the complete filesystem.
pub(super) fn detect(source: &File) -> io::Result<&'static str> {
    let mut block = [0; 4096];
    source.read_exact_at(&mut block, 0)?;
    if &block[..4] == b"XFSB" {
        return Ok("xfs");
    }
    if block[1080..1082] == [0x53, 0xef] {
        return Ok("ext4");
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "root is not ext4 or XFS",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selects_only_supported_filesystem_signatures() {
        for (kind, offset, magic) in [("ext4", 1080, &[0x53, 0xef][..]), ("xfs", 0, &b"XFSB"[..])] {
            let file = tempfile::tempfile().unwrap();
            file.set_len(128 * 1024).unwrap();
            assert!(detect(&file).is_err());
            file.write_all_at(magic, offset).unwrap();
            assert_eq!(detect(&file).unwrap(), kind);
        }
    }
}
