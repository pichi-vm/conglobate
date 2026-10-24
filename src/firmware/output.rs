// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! Immutable firmware captured from a quiescent tool filesystem.
use super::{DtbContract, FirmwareError};
use crate::{
    recipe::TransformSelection,
    step::SetupBudget,
    storage::{Scratch, Workspace},
    transform::{TransformError, TransformedDtb},
};
use rustix::fs::{self, Mode, OFlags, ResolveFlags};
use sha2::{Digest as _, Sha256};
use std::{
    fs::File,
    io::{self, Read, Write},
    os::fd::AsRawFd,
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum ExtractionError {
    #[error("firmware extraction: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Transform(#[from] TransformError),
    #[error(transparent)]
    Contract(#[from] FirmwareError),
}
/// Both declared files have been captured. No output filesystem authority escapes.
#[derive(Debug)]
pub(crate) struct ExtractedFirmware<'w> {
    pmi: FirmwareFile<'w>,
    dtb: FirmwareFile<'w>,
}
/// Final DTB bytes and their compatible firmware. This is not command success.
#[derive(Debug)]
pub(crate) struct FinalizedFirmware<'w> {
    pmi: FirmwareFile<'w>,
    dtb: TransformedDtb,
}
#[derive(Debug)]
struct FirmwareFile<'w> {
    bytes: Scratch<'w>,
    digest: pichi_artifact::Digest,
    size: u64,
}
impl<'w> ExtractedFirmware<'w> {
    pub(crate) fn capture(
        root: &File,
        pmi: &str,
        dtb: &str,
        workspace: &'w Workspace,
        budget: &SetupBudget,
    ) -> Result<Self, ExtractionError> {
        let pmi = FirmwareFile::open(root, pmi)?;
        let dtb = FirmwareFile::open(root, dtb)?;
        Ok(Self {
            pmi: FirmwareFile::capture(pmi, workspace, budget)?,
            dtb: FirmwareFile::capture(dtb, workspace, budget)?,
        })
    }
    pub(crate) fn finalize(
        self,
        transform: &TransformSelection,
        carapace: Option<&str>,
    ) -> Result<FinalizedFirmware<'w>, ExtractionError> {
        let original = self.dtb.read()?;
        let dtb = transform.apply(Some(&original), carapace)?;
        if dtb.bytes() != Some(original.as_slice()) {
            DtbContract::from_pmi(&self.pmi.read()?)?
                .check_size(dtb.bytes().expect("present input DTB").len())?;
        }
        Ok(FinalizedFirmware { pmi: self.pmi, dtb })
    }
}
impl FinalizedFirmware<'_> {
    pub(crate) fn pmi_descriptor(&self) -> pichi_artifact::PmiDescriptor {
        pichi_artifact::PmiDescriptor {
            digest: self.pmi.digest.to_string(),
            size: self.pmi.size,
        }
    }
    pub(crate) fn pmi_reader(&self) -> impl Read + '_ {
        self.pmi.reader()
    }
    pub(crate) fn dtb(&self) -> &TransformedDtb {
        &self.dtb
    }
}
impl<'w> FirmwareFile<'w> {
    fn open(root: &File, path: &str) -> io::Result<File> {
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
    fn capture(
        mut source: File,
        workspace: &'w Workspace,
        budget: &SetupBudget,
    ) -> io::Result<Self> {
        budget.check()?;
        let size = source.metadata()?.len();
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
    fn reader(&self) -> crate::storage::ScuteReader<'_> {
        crate::storage::ScuteReader::new(self.bytes.file(), self.size)
    }
    fn read(&self) -> io::Result<Vec<u8>> {
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
