// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    step::SetupBudget,
    storage::{Scratch, Workspace},
};
use rustix::fs::{self, Mode, OFlags, ResolveFlags};
use sha2::{Digest as _, Sha256};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::fd::AsRawFd,
};

#[derive(Debug)]
pub(crate) struct FirmwareFile<'w> {
    bytes: Scratch<'w>,
    digest: crate::artifact::Digest,
    size: u64,
}
impl<'w> FirmwareFile<'w> {
    pub(crate) fn open(root: &File, path: &str) -> io::Result<File> {
        let observed: File = fs::openat2(
            root,
            path,
            OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_XDEV,
        )?
        .into();
        if !observed.metadata()?.is_file() {
            return Err(io::Error::other("firmware output is not a regular file"));
        }
        // Reopen the pinned, checked regular inode, not the image-controlled path.
        File::open(format!("/proc/self/fd/{}", observed.as_raw_fd()))
    }
    pub(crate) fn capture(
        mut source: File,
        workspace: &'w Workspace,
        budget: &SetupBudget,
    ) -> io::Result<Self> {
        budget.check()?;
        let metadata = source.metadata()?;
        let size = metadata.len();
        let mut bytes = workspace.scratch()?;
        let mut hash = Sha256::new();
        let mut remaining = size;
        let mut buffer = [0; 65536];
        while remaining != 0 {
            budget.check()?;
            let count = buffer.len().min(remaining as usize);
            let count = source.read(&mut buffer[..count])?;
            if count == 0 {
                return Err(io::Error::other(
                    "firmware output truncated during extraction",
                ));
            }
            bytes.write_all(&buffer[..count])?;
            hash.update(&buffer[..count]);
            remaining -= count as u64;
        }
        if source.read(&mut buffer[..1])? != 0 {
            return Err(io::Error::other("firmware output grew during extraction"));
        }
        bytes.sync_all()?;
        budget.check()?;
        let digest = format!("sha256:{}", hex::encode(hash.finalize()))
            .parse()
            .map_err(|_| io::Error::other("invalid captured digest"))?;
        Ok(Self {
            bytes,
            digest,
            size,
        })
    }
    pub(crate) fn descriptor(&self) -> crate::artifact::PmiDescriptor {
        crate::artifact::PmiDescriptor {
            digest: self.digest.to_string(),
            size: self.size,
        }
    }
    pub(crate) fn reader(&self) -> crate::storage::ScuteReader<'_> {
        crate::storage::ScuteReader::new(self.bytes.file(), self.size)
    }
    pub(crate) fn read(&self) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(usize::try_from(self.size).map_err(io::Error::other)?)
            .map_err(io::Error::other)?;
        self.reader().read_to_end(&mut bytes)?;
        Ok(bytes)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    #[test]
    fn image_paths_resolve_inside_root_and_special_files_are_never_opened() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("pmi"), b"inside").unwrap();
        symlink("/pmi", directory.path().join("absolute")).unwrap();
        symlink("../../pmi", directory.path().join("parent")).unwrap();
        symlink("/etc/passwd", directory.path().join("outside")).unwrap();
        symlink("cycle", directory.path().join("cycle")).unwrap();
        let root = File::open(directory.path()).unwrap();
        for path in ["/pmi", "absolute", "parent"] {
            let mut bytes = Vec::new();
            FirmwareFile::open(&root, path)
                .unwrap()
                .read_to_end(&mut bytes)
                .unwrap();
            assert_eq!(bytes, b"inside");
        }
        for path in ["outside", "cycle", ".", "missing"] {
            assert!(FirmwareFile::open(&root, path).is_err());
        }
        fs::mknodat(
            &root,
            "fifo",
            fs::FileType::Fifo,
            Mode::RUSR | Mode::WUSR,
            0,
        )
        .unwrap();
        assert!(
            FirmwareFile::open(&root, "fifo")
                .unwrap_err()
                .to_string()
                .contains("regular file")
        );
    }
}
