// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{MountLease, Point};
use crate::{
    anchor::RecoveryDirectory, contract::plan::ImagePath, isolation::IdRange, planner::Mount,
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
    ) -> io::Result<()> {
        if !self.filesystems.is_empty() {
            return Err(io::Error::other("stage mounts are already configured"));
        }
        // The planner validates mount paths before any storage is activated.
        let mut ordered = mounts.iter().collect::<Vec<_>>();
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
                    let mode = match path.normalized().as_str() {
                        "/run" | "/var" => "0755",
                        _ => "1777",
                    };
                    mount::fsconfig_set_string(&context, "mode", mode)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::YamlDocument,
        planner::Plan,
        storage::{Created, Mapping, Workspace},
    };
    use std::{
        num::NonZeroU64,
        os::unix::fs::{MetadataExt, PermissionsExt},
    };

    #[test]
    #[ignore = "requires root, tmpfs, loop devices and device mapper"]
    fn defaults_survive_views_but_not_stage_completion() {
        let parent = tempfile::tempdir_in("/dev/shm").unwrap();
        std::fs::set_permissions(parent.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let scope = RecoveryDirectory::open(parent.path()).unwrap();
        let workspace = Workspace::create().unwrap();
        let sectors = NonZeroU64::new(carapace::ZERO_SIZE_SECTORS).unwrap();
        let origin = Mapping::<Created>::create().unwrap().zero(sectors).unwrap();
        let snapshot = Mapping::<Created>::create()
            .unwrap()
            .snapshot(
                &origin,
                workspace.snapshot_store().unwrap().attach().unwrap(),
                sectors,
            )
            .unwrap();
        let snapshot = PartitionedSnapshot::open(snapshot).unwrap();
        let plan = Plan::try_from(YamlDocument::from(
            "version: 1\nimage: x\nstages: {x: {extend: registry.test/base:1}}",
        ))
        .unwrap();
        let mounts = &plan.stages.values().next().unwrap().mount;
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("var")).unwrap();
        std::fs::write(root.path().join("var/inherited"), "keep").unwrap();
        let root_file = File::open(root.path()).unwrap();
        let mut stage = StageMounts::default();
        stage
            .populate(mounts, &snapshot, &scope, parent.path())
            .unwrap();
        for iteration in 0..2 {
            let mut attachments = StageAttachments::default();
            stage.attach(&root_file, &scope, &mut attachments).unwrap();
            for (path, mode) in [("run", 0o755), ("tmp", 0o1777), ("var", 0o755)] {
                let path = root.path().join(path);
                assert_eq!(path.metadata().unwrap().mode() & 0o7777, mode);
                assert!(!path.join("inherited").exists());
                if iteration == 0 {
                    std::fs::write(path.join("temporary"), "same stage").unwrap();
                } else {
                    assert_eq!(
                        std::fs::read_to_string(path.join("temporary")).unwrap(),
                        "same stage"
                    );
                }
            }
            attachments.close(&scope).unwrap();
        }
        stage.close(&scope).unwrap();
        for path in ["run", "tmp", "var"] {
            assert!(!root.path().join(path).join("temporary").exists());
        }
        assert_eq!(
            std::fs::read_to_string(root.path().join("var/inherited")).unwrap(),
            "keep"
        );
        snapshot.into_snapshot().unwrap().close().unwrap();
        origin.close().unwrap();
    }
}
