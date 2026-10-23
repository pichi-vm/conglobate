// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use rustix::{fd::OwnedFd, fs};
use std::io;

/// Selected from trusted volume handoff, never from image directory contents.
#[derive(Debug, Clone, Copy)]
pub(crate) enum AssignedRoots {
    Root,
    Bootstrap,
}
impl AssignedRoots {
    pub(crate) fn tag(self) -> u8 {
        match self {
            Self::Root => 0,
            Self::Bootstrap => 1,
        }
    }
    pub(crate) fn verify_labels(self) -> io::Result<()> {
        let root = fs::open(
            "/",
            fs::OFlags::PATH | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC,
            fs::Mode::empty(),
        )?;
        LabelProbe::new(&root)
            .and_then(LabelProbe::verify)
            .map_err(|error| io::Error::other(format!("{error} (command root /)")))?;
        if let Self::Bootstrap = self {
            let output = fs::openat2(
                &root,
                "mnt",
                fs::OFlags::PATH | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC,
                fs::Mode::empty(),
                fs::ResolveFlags::BENEATH | fs::ResolveFlags::NO_SYMLINKS,
            )
            .map_err(|error| {
                io::Error::other(format!(
                    "open output for labeling: {error} (command root /mnt)"
                ))
            })?;
            LabelProbe::new(&output)
                .and_then(LabelProbe::verify)
                .map_err(|error| io::Error::other(format!("{error} (command root /mnt)")))?;
        }
        Ok(())
    }
}
impl TryFrom<u8> for AssignedRoots {
    type Error = io::Error;
    fn try_from(value: u8) -> io::Result<Self> {
        match value {
            0 => Ok(Self::Root),
            1 => Ok(Self::Bootstrap),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid assigned-root layout",
            )),
        }
    }
}

/// Anonymous inode owned only during trusted restricted setup. No filename or
/// descriptor survives admission; final storage flushing remains independent.
#[derive(Debug)]
struct LabelProbe(OwnedFd);
impl LabelProbe {
    fn new(root: &OwnedFd) -> io::Result<Self> {
        Ok(Self(
            fs::openat(
                root,
                ".",
                fs::OFlags::TMPFILE | fs::OFlags::EXCL | fs::OFlags::RDWR | fs::OFlags::CLOEXEC,
                fs::Mode::RUSR | fs::Mode::WUSR,
            )
            .map_err(|error| io::Error::other(format!("create anonymous label probe: {error}")))?,
        ))
    }
    fn verify(self) -> io::Result<()> {
        const LABEL: &[u8] = b"conglobate_probe_u:conglobate_probe_r:conglobate_probe_t:s0\0";
        fs::fsetxattr(&self.0, "security.selinux", LABEL, fs::XattrFlags::empty()).map_err(
            |e| {
                io::Error::other(format!(
                    "environment labeling support: {e} while writing security.selinux"
                ))
            },
        )?;
        let mut actual = [0u8; LABEL.len()];
        let count = fs::fgetxattr(&self.0, "security.selinux", &mut actual[..])
            .map_err(|error| io::Error::other(format!("read environment label: {error}")))?;
        if count != LABEL.len() || actual != LABEL {
            return Err(io::Error::other("environment label readback mismatch"));
        }
        fs::fremovexattr(&self.0, "security.selinux")
            .map_err(|error| io::Error::other(format!("remove environment label: {error}")))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AssignedRoots;

    #[test]
    fn rejects_unknown_root_layouts() {
        for tag in 2..=u8::MAX {
            assert_eq!(
                AssignedRoots::try_from(tag).unwrap_err().kind(),
                std::io::ErrorKind::InvalidData
            );
        }
    }
}
