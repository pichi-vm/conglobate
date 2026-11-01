// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CopyDestination, CopyError, MetadataPolicy};
use crate::{plan::Copy, step::SetupBudget};
use rustix::fs::{self, AtFlags, Mode, OFlags, ResolveFlags};
use std::{
    fs::File,
    io::{Read, Write},
    os::{fd::AsRawFd, unix::fs::PermissionsExt},
    path::{Path, PathBuf},
};

#[derive(Debug)]
struct Directory {
    file: File,
    entries: fs::Dir,
    target: PathBuf,
}
impl Directory {
    fn open(file: File, target: PathBuf) -> Result<Self, CopyError> {
        let file = File::open(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
        let entries = fs::Dir::read_from(&file)?;
        Ok(Self {
            file,
            entries,
            target,
        })
    }
}
impl CopyDestination<'_> {
    /// Read a persisted source namespace. O_PATH observations reject devices
    /// and FIFOs before any data open; nested symlinks remain symlinks.
    pub(crate) fn image(
        &self,
        root: &File,
        directive: &Copy,
        budget: &SetupBudget,
    ) -> Result<(), CopyError> {
        budget.check()?;
        let source: File = fs::openat2(
            root,
            directive.path().as_ref(),
            OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_XDEV,
        )?
        .into();
        let metadata = source.metadata()?;
        if !metadata.is_dir() && !metadata.is_file() {
            return Err(CopyError::Invalid(
                "COPY source is not a directory or regular file",
            ));
        }
        let policy = self.policy(directive)?;
        let mut target = self.resolve(Path::new(directive.destination()), true)?;
        if directive.destination().ends_with('/') {
            self.directory(&target, &policy)?;
        }
        if !metadata.is_dir() {
            if self
                .observe(&target)?
                .is_some_and(|metadata| metadata.is_dir())
            {
                target.push(
                    Path::new(directive.path().as_ref())
                        .file_name()
                        .ok_or(CopyError::Invalid("COPY source has no filename"))?,
                );
            }
            return self.image_leaf(&source, &target, &policy, budget);
        }
        self.directory(&target, &policy)?;
        let mut pending = vec![Directory::open(source, target)?];
        while let Some(directory) = pending.last_mut() {
            budget.check()?;
            let Some(entry) = directory.entries.next() else {
                pending.pop();
                continue;
            };
            let entry = entry?;
            let name = entry.file_name();
            if matches!(name.to_bytes(), b"." | b"..") {
                continue;
            }
            let source: File = fs::openat2(
                &directory.file,
                name,
                OFlags::PATH | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
            )?
            .into();
            let target = self.resolve(
                &directory
                    .target
                    .join(std::ffi::OsStr::from_bytes(name.to_bytes())),
                false,
            )?;
            if source.metadata()?.is_dir() {
                self.directory(&target, &policy)?;
                pending.push(Directory::open(source, target)?);
            } else {
                self.image_leaf(&source, &target, &policy, budget)?;
            }
        }
        budget.check()?;
        Ok(())
    }
    fn image_leaf(
        &self,
        source: &File,
        target: &Path,
        policy: &MetadataPolicy<'_>,
        budget: &SetupBudget,
    ) -> Result<(), CopyError> {
        let metadata = source.metadata()?;
        if !metadata.is_file() && !metadata.is_symlink() {
            return Err(CopyError::Invalid("COPY cannot read special image files"));
        }
        if self
            .observe(target)?
            .is_some_and(|metadata| metadata.is_dir())
        {
            return Err(CopyError::Invalid("COPY file conflicts with directory"));
        }
        let name = target
            .file_name()
            .ok_or(CopyError::Invalid("COPY file destination is root"))?;
        let parent = self.directory(target.parent().unwrap(), policy)?;
        match fs::unlinkat(&parent, name, AtFlags::empty()) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => (),
            Err(error) => return Err(error.into()),
        }
        if metadata.is_symlink() {
            let link = fs::readlinkat(source, "", Vec::new())?;
            fs::symlinkat(link.as_c_str(), &parent, name)?;
            policy.link(&parent, name)?;
        } else {
            let mut input = File::open(format!("/proc/self/fd/{}", source.as_raw_fd()))?;
            let mut output: File = fs::openat(
                &parent,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::from_raw_mode(0o600),
            )?
            .into();
            let mut remaining = metadata.len();
            let mut buffer = [0; 65536];
            while remaining > 0 {
                budget.check()?;
                let count = remaining.min(buffer.len() as u64) as usize;
                let count = input.read(&mut buffer[..count])?;
                if count == 0 {
                    return Err(CopyError::Invalid("COPY source was truncated"));
                }
                output.write_all(&buffer[..count])?;
                remaining -= count as u64;
            }
            if input.read(&mut buffer[..1])? != 0 {
                return Err(CopyError::Invalid("COPY source grew during reading"));
            }
            policy.file(
                &output,
                if metadata.permissions().mode() & 0o111 != 0 {
                    0o755
                } else {
                    0o644
                },
                false,
            )?;
        }
        budget.check()?;
        Ok(())
    }
}
use std::os::unix::ffi::OsStrExt;

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn directive(path: &str, into: &str) -> Copy {
        Copy::fixture(
            path,
            into,
            rustix::process::getuid().as_raw().to_string(),
            rustix::process::getgid().as_raw().to_string(),
            None,
        )
    }
    #[test]
    fn stage_files_follow_selected_links_and_preserve_nested_links() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        std::fs::create_dir(source.path().join("tree")).unwrap();
        std::fs::write(source.path().join("tree/plain"), b"content").unwrap();
        std::fs::write(source.path().join("tree/exec"), b"#!/bin/sh\n").unwrap();
        std::fs::set_permissions(
            source.path().join("tree/exec"),
            std::fs::Permissions::from_mode(0o4751),
        )
        .unwrap();
        symlink("plain", source.path().join("tree/link")).unwrap();
        symlink("/tree", source.path().join("selected")).unwrap();
        symlink("/tree/plain", source.path().join("file")).unwrap();
        let root = File::open(source.path()).unwrap();
        let target = File::open(destination.path()).unwrap();
        let copy = CopyDestination::new(&target);
        copy.image(
            &root,
            &directive("/selected", "/output/"),
            &SetupBudget::begin(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(destination.path().join("output/plain")).unwrap(),
            b"content"
        );
        assert_eq!(
            std::fs::read_link(destination.path().join("output/link")).unwrap(),
            Path::new("plain")
        );
        assert_eq!(
            std::fs::metadata(destination.path().join("output/exec"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o755
        );
        assert_eq!(
            std::fs::metadata(destination.path().join("output/plain"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o644
        );
        copy.image(&root, &directive("/file", "/files/"), &SetupBudget::begin())
            .unwrap();
        assert_eq!(
            std::fs::read(destination.path().join("files/file")).unwrap(),
            b"content"
        );
        copy.image(&root, &directive("/", "/whole"), &SetupBudget::begin())
            .unwrap();
        assert!(destination.path().join("whole/tree/plain").is_file());
        assert!(!destination.path().join("whole/whole").exists());
    }
    #[test]
    fn stage_copy_confines_source_and_destination_and_breaks_hardlinks() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        std::fs::write(external.path().join("secret"), b"host").unwrap();
        symlink(external.path().join("secret"), source.path().join("escape")).unwrap();
        std::fs::write(source.path().join("new"), b"new").unwrap();
        std::fs::create_dir(destination.path().join("inside")).unwrap();
        symlink("/inside", destination.path().join("absolute")).unwrap();
        std::fs::hard_link(
            external.path().join("secret"),
            destination.path().join("inside/new"),
        )
        .unwrap();
        let root = File::open(source.path()).unwrap();
        let target = File::open(destination.path()).unwrap();
        let copy = CopyDestination::new(&target);
        assert!(
            copy.image(
                &root,
                &directive("escape", "/unpublished"),
                &SetupBudget::begin()
            )
            .is_err()
        );
        assert!(!destination.path().join("unpublished").exists());
        copy.image(
            &root,
            &directive("new", "/absolute/"),
            &SetupBudget::begin(),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(destination.path().join("inside/new")).unwrap(),
            b"new"
        );
        assert_eq!(
            std::fs::read(external.path().join("secret")).unwrap(),
            b"host"
        );
        fs::mknodat(
            &root,
            "pipe",
            fs::FileType::Fifo,
            Mode::RUSR | Mode::WUSR,
            0,
        )
        .unwrap();
        assert!(
            copy.image(&root, &directive("pipe", "/pipe"), &SetupBudget::begin())
                .is_err()
        );
        assert!(!destination.path().join("pipe").exists());
        assert!(
            copy.image(&root, &directive("/", "/nested"), &SetupBudget::begin())
                .is_err()
        );
    }
}
