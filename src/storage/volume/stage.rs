// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{MountLease, Point};
use crate::{
    anchor::RecoveryDirectory,
    contract::recipe::{ImagePath, Mount},
    isolation::IdRange,
    storage::PartitionedSnapshot,
};
use rustix::{fs, mount};
use std::{collections::BTreeMap, fs::File, io, os::fd::AsRawFd, path::Path, time::Instant};

#[derive(Debug)]
struct StageFilesystem {
    path: ImagePath,
    kind: Mount,
    root: File,
    lease: MountLease,
}

/// Explicit filesystem owners retained across all directives in one stage.
/// They are attached only under trusted supervisor storage until views are
/// cloned into a command or COPY destination. They are never image exports.
#[derive(Debug, Default)]
pub(super) struct StageMounts {
    filesystems: Vec<StageFilesystem>,
}

impl StageMounts {
    pub(super) fn populate(
        &mut self,
        mounts: &BTreeMap<ImagePath, Mount>,
        snapshot: &PartitionedSnapshot<'_, '_>,
        scope: &RecoveryDirectory,
        parent: &Path,
        creation: bool,
    ) -> io::Result<()> {
        if !self.filesystems.is_empty() {
            return Err(io::Error::other("stage mounts are already configured"));
        }
        let mut paths = BTreeMap::new();
        for (path, kind) in mounts {
            let normalized = path.normalized();
            let first = normalized
                .trim_start_matches('/')
                .split('/')
                .next()
                .unwrap_or("");
            if normalized == "/"
                || (!creation && matches!(first, "dev" | "proc" | "sys"))
                || paths.insert(normalized, (path, kind)).is_some()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "stage mount conflicts with root, runtime controls or another mount",
                ));
            }
        }
        let mut ordered = paths.into_values().collect::<Vec<_>>();
        ordered.sort_by_key(|(path, _)| path.normalized().split('/').count());
        for (path, &kind) in ordered {
            let filesystem = (|| -> io::Result<StageFilesystem> {
                scope.verify_current()?;
                let point = Point::create(parent)?;
                let context = mount::fsopen(
                    match kind {
                        Mount::Esp => "vfat",
                        Mount::Tmpfs => "tmpfs",
                    },
                    mount::FsOpenFlags::FSOPEN_CLOEXEC,
                )?;
                let source = match kind {
                    Mount::Esp => Some(snapshot.open_esp()?),
                    Mount::Tmpfs => None,
                };
                if let Some(source) = &source {
                    mount::fsconfig_set_string(
                        &context,
                        "source",
                        format!("/proc/self/fd/{}", source.as_raw_fd()),
                    )?;
                    mount::fsconfig_set_string(&context, "umask", "0022")?;
                } else {
                    mount::fsconfig_set_string(&context, "mode", "1777")?;
                }
                mount::fsconfig_set_string(&context, "uid", "0")?;
                mount::fsconfig_set_string(&context, "gid", "0")?;
                mount::fsconfig_create(&context)?;
                let tree = mount::fsmount(
                    &context,
                    mount::FsMountFlags::FSMOUNT_CLOEXEC,
                    mount::MountAttrFlags::MOUNT_ATTR_NOSUID
                        | mount::MountAttrFlags::MOUNT_ATTR_NODEV,
                )?;
                let root = File::from(fs::openat(
                    &tree,
                    ".",
                    fs::OFlags::RDONLY | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC,
                    fs::Mode::empty(),
                )?);
                mount::move_mount(
                    &tree,
                    "",
                    &point.parent,
                    &point.name,
                    mount::MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
                )?;
                Ok(StageFilesystem {
                    path: path.clone(),
                    kind,
                    root,
                    lease: MountLease { point: Some(point) },
                })
            })();
            self.filesystems.push(filesystem?);
        }
        Ok(())
    }

    pub(super) fn attach(
        &self,
        root: &File,
        scope: &RecoveryDirectory,
        attachments: &mut StageAttachments,
    ) -> io::Result<()> {
        for filesystem in &self.filesystems {
            let attachment = (|| -> io::Result<(File, MountLease)> {
                scope.verify_current()?;
                let point = Point::image(root, &filesystem.path.normalized())?;
                let tree = mount::open_tree(
                    &filesystem.root,
                    "",
                    mount::OpenTreeFlags::AT_EMPTY_PATH
                        | mount::OpenTreeFlags::OPEN_TREE_CLONE
                        | mount::OpenTreeFlags::OPEN_TREE_CLOEXEC,
                )?;
                let file = File::from(fs::openat(
                    &tree,
                    ".",
                    fs::OFlags::RDONLY | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC,
                    fs::Mode::empty(),
                )?);
                mount::move_mount(
                    &tree,
                    "",
                    &point.parent,
                    &point.name,
                    mount::MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
                )?;
                Ok((file, MountLease { point: Some(point) }))
            })();
            attachments.points.push(attachment?);
        }
        Ok(())
    }

    pub(super) fn validate_ownership(
        &self,
        uid: IdRange,
        gid: IdRange,
        deadline: Instant,
    ) -> io::Result<()> {
        for filesystem in &self.filesystems {
            if filesystem.kind == Mount::Tmpfs {
                crate::storage::ownership::ImageOwnership::new(uid, gid, deadline)
                    .validate(&filesystem.root)?;
            }
            // FAT has no per-inode Unix owners or POSIX ACLs. Its filesystem
            // owner is fixed at image UID/GID 0 by the trusted mount above.
        }
        Ok(())
    }

    pub(super) fn close(mut self, scope: &RecoveryDirectory) -> io::Result<()> {
        while let Some(filesystem) = self.filesystems.pop() {
            if let Err(error) = filesystem.lease.close(filesystem.root, scope) {
                std::mem::forget(self);
                return Err(error);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub(super) struct StageAttachments {
    points: Vec<(File, MountLease)>,
}
impl StageAttachments {
    pub(super) fn close(mut self, scope: &RecoveryDirectory) -> io::Result<()> {
        while let Some((root, lease)) = self.points.pop() {
            if let Err(error) = lease.close(root, scope) {
                std::mem::forget(self);
                return Err(error);
            }
        }
        Ok(())
    }
}
impl Drop for StageAttachments {
    fn drop(&mut self) {
        while let Some(point) = self.points.pop() {
            // Fallback only. MountLease's lazy detach never yields a receipt.
            drop(point);
        }
    }
}
