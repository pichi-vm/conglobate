// SPDX-License-Identifier: Apache-2.0
//! Check the original, quiescent filesystem before creating command views.
use crate::isolation::IdRange;
use rustix::fs::{self, Mode, OFlags, ResolveFlags};
use std::{
    fs::{File, ReadDir},
    io,
    os::{fd::AsRawFd, unix::fs::MetadataExt},
    path::PathBuf,
    time::Instant,
};

#[derive(Debug)]
pub(super) struct ImageOwnership {
    uid: IdRange,
    gid: IdRange,
    deadline: Instant,
}
#[derive(Debug)]
struct Directory {
    file: File,
    entries: ReadDir,
}
impl Directory {
    fn open(file: File) -> io::Result<Self> {
        let entries = std::fs::read_dir(ImageOwnership::descriptor_path(&file))?;
        Ok(Self { file, entries })
    }
}
impl ImageOwnership {
    pub(super) fn new(uid: IdRange, gid: IdRange, deadline: Instant) -> Self {
        Self { uid, gid, deadline }
    }
    fn check_deadline(&self) -> io::Result<()> {
        if Instant::now() >= self.deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "image ownership validation",
            ));
        }
        Ok(())
    }
    fn descriptor_path(file: &File) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
    }
    fn owner(&self, id: u32, group: bool) -> io::Result<()> {
        let range = if group { self.gid } else { self.uid };
        if !range.contains_image_id(id) {
            let kind = if group { "GID" } else { "UID" };
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unmapped image {kind} {id}"),
            ));
        }
        Ok(())
    }
    fn acl(&self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() < 4
            || bytes[..4] != 2u32.to_le_bytes()
            || !(bytes.len() - 4).is_multiple_of(8)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid POSIX ACL encoding",
            ));
        }
        for entry in bytes[4..].as_chunks::<8>().0 {
            let tag = u16::from_le_bytes([entry[0], entry[1]]);
            if u16::from_le_bytes([entry[2], entry[3]]) > 7 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid POSIX ACL permissions",
                ));
            }
            let id = u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]);
            match tag {
                2 => self.owner(id, false)?,
                8 => self.owner(id, true)?,
                1 | 4 | 16 | 32 if id == u32::MAX => (),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "invalid POSIX ACL tag or ID",
                    ));
                }
            }
        }
        Ok(())
    }
    fn inode(&self, file: &File, buffer: &mut [u8]) -> io::Result<()> {
        self.check_deadline()?;
        let metadata = file.metadata()?;
        self.owner(metadata.uid(), false)?;
        self.owner(metadata.gid(), true)?;
        // Linux symlinks have no POSIX ACL. Never follow image symlinks while
        // inspecting metadata. O_PATH also avoids activating devices or FIFOs.
        if !metadata.is_symlink() {
            for name in ["system.posix_acl_access", "system.posix_acl_default"] {
                // fgetxattr rejects O_PATH. The supervisor's trusted procfs
                // resolves this pinned inode without opening it for device I/O.
                match fs::getxattr(Self::descriptor_path(file), name, &mut *buffer) {
                    Ok(count) => self.acl(&buffer[..count])?,
                    Err(rustix::io::Errno::NODATA) => (),
                    Err(error) => return Err(error.into()),
                }
            }
        }
        self.check_deadline()
    }
    pub(super) fn validate(&self, root: &File) -> io::Result<()> {
        let mut buffer = vec![0; 65536]; // Linux's maximum xattr value size.
        self.inode(root, &mut buffer)?;
        let mut pending = vec![Directory::open(root.try_clone()?)?];
        while let Some(directory) = pending.last_mut() {
            self.check_deadline()?;
            let Some(entry) = directory.entries.next() else {
                pending.pop();
                continue;
            };
            let name = entry?.file_name();
            let file: File = fs::openat2(
                &directory.file,
                name,
                OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
            )?
            .into();
            self.inode(&file, &mut buffer)?;
            if file.metadata()?.is_dir() {
                pending.push(Directory::open(file)?);
            }
        }
        self.check_deadline()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traversal_does_not_follow_links_or_open_fifos_and_honors_deadline() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::create_dir(directory.path().join("nested")).unwrap();
        std::fs::write(directory.path().join("nested/file"), b"test").unwrap();
        std::os::unix::fs::symlink("/missing-image-target", directory.path().join("link")).unwrap();
        fs::mknodat(
            fs::CWD,
            directory.path().join("fifo"),
            fs::FileType::Fifo,
            Mode::RUSR | Mode::WUSR,
            0,
        )
        .unwrap();
        let root = File::open(directory.path()).unwrap();
        let metadata = root.metadata().unwrap();
        let uid = IdRange::new(1, metadata.uid() + 1).unwrap();
        let gid = IdRange::new(1, metadata.gid() + 1).unwrap();
        ImageOwnership::new(uid, gid, Instant::now() + std::time::Duration::from_secs(5))
            .validate(&root)
            .unwrap();
        assert_eq!(
            ImageOwnership::new(uid, gid, Instant::now())
                .validate(&root)
                .unwrap_err()
                .kind(),
            io::ErrorKind::TimedOut
        );
    }
    #[test]
    fn acl_named_ids_respect_separate_ranges_and_encoding() {
        let policy = ImageOwnership::new(
            IdRange::new(100000, 65536).unwrap(),
            IdRange::new(200000, 100).unwrap(),
            Instant::now(),
        );
        for (tag, id, valid) in [
            (2u16, 65535u32, true),
            (2, 65536, false),
            (8, 99, true),
            (8, 100, false),
            (1, u32::MAX, true),
            (1, 0, false),
            (64, 0, false),
        ] {
            let mut bytes = 2u32.to_le_bytes().to_vec();
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&7u16.to_le_bytes());
            bytes.extend_from_slice(&id.to_le_bytes());
            assert_eq!(policy.acl(&bytes).is_ok(), valid);
        }
        for bytes in [&[][..], &[2, 0, 0], &[3, 0, 0, 0], &[2, 0, 0, 0, 0]] {
            assert!(policy.acl(bytes).is_err());
        }
    }
}
