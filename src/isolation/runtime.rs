// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use rustix::{
    fs::{AtFlags, Mode, OFlags, ResolveFlags},
    mount::{
        self, FsMountFlags, FsOpenFlags, MountAttr, MountAttrFlags, MountPropagationFlags,
        MountSetAttrFlags, MoveMountFlags, OpenTreeFlags,
    },
};
use std::{fs::File, io, os::unix::fs::MetadataExt};

/// Targets pinned before mounting either runtime filesystem. Resolution treats
/// absolute symlinks as image-relative and rejects proc-style magic links.
#[derive(Debug)]
pub(crate) struct RuntimeTargets {
    proc: File,
    sys: File,
    dev: File,
}
impl RuntimeTargets {
    pub(crate) fn resolve(root: &File) -> io::Result<Self> {
        let dev = Self::directory(root, "dev")?;
        let proc = Self::directory(root, "proc")?;
        let sys = Self::directory(root, "sys")?;
        let root_id = root.metadata()?;
        let proc_id = proc.metadata()?;
        let sys_id = sys.metadata()?;
        let root_key = (root_id.dev(), root_id.ino());
        let proc_key = (proc_id.dev(), proc_id.ino());
        let sys_key = (sys_id.dev(), sys_id.ino());
        if proc_key == root_key || sys_key == root_key || proc_key == sys_key {
            return Err(io::Error::other(
                "runtime mount targets alias root or each other",
            ));
        }
        Self::reject_nested(&proc, sys_key, root_key)?;
        Self::reject_nested(&sys, proc_key, root_key)?;
        let dev_id = dev.metadata()?;
        let dev_key = (dev_id.dev(), dev_id.ino());
        if [root_key, proc_key, sys_key].contains(&dev_key) {
            return Err(io::Error::other(
                "device target aliases another runtime target",
            ));
        }
        for (other, key) in [(&proc, proc_key), (&sys, sys_key)] {
            Self::reject_nested(&dev, key, root_key)?;
            Self::reject_nested(other, dev_key, root_key)?;
        }
        Ok(Self { proc, sys, dev })
    }

    fn reject_nested(directory: &File, other: (u64, u64), root: (u64, u64)) -> io::Result<()> {
        let mut current = directory.try_clone()?;
        // No commands have been admitted; the trusted caller retains exclusive
        // ownership of this image view. Bound malformed/deep-tree processing.
        for _ in 0..4096 {
            let parent: File = rustix::fs::openat(
                &current,
                "..",
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
            )?
            .into();
            let metadata = parent.metadata()?;
            let key = (metadata.dev(), metadata.ino());
            if key == other {
                return Err(io::Error::other("runtime mount targets are nested"));
            }
            if key == root {
                return Ok(());
            }
            let before = current.metadata()?;
            if key == (before.dev(), before.ino()) {
                break;
            }
            current = parent;
        }
        Err(io::Error::other(
            "runtime target ancestry is not bounded by image root",
        ))
    }

    /// Open `path` as a directory mount point. A missing final component is
    /// created on the writable image: bare roots (the context-recipe smoke
    /// carapace has neither `/dev` nor `/proc` nor `/sys`) otherwise fail
    /// `openat2` with `ENOENT` before any command runs. Existing symlinks and
    /// non-directories are left alone and still fail closed.
    fn directory(root: &File, path: &str) -> io::Result<File> {
        let opened = match Self::open_directory(root, path) {
            Ok(file) => Ok(file),
            Err(rustix::io::Errno::NOENT) => {
                match rustix::fs::mkdirat(root, path, Mode::from_raw_mode(0o755)) {
                    Ok(()) => {
                        // umask must not leave the mount point non-traversable.
                        let _ = rustix::fs::chmodat(
                            root,
                            path,
                            Mode::from_raw_mode(0o755),
                            AtFlags::empty(),
                        );
                        Self::open_directory(root, path)
                    }
                    // A dangling symlink or other non-directory occupies the name.
                    Err(rustix::io::Errno::EXIST) => Self::open_directory(root, path),
                    Err(error) => Err(error),
                }
            }
            Err(error) => Err(error),
        };
        opened.map_err(|error| {
            io::Error::new(
                io::Error::from(error).kind(),
                format!("command runtime target /{path}: {error}"),
            )
        })
    }

    fn open_directory(root: &File, path: &str) -> Result<File, rustix::io::Errno> {
        Ok(rustix::fs::openat2(
            root,
            path,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_XDEV,
        )?
        .into())
    }

    /// Call only in the private command setup mount/PID namespaces. A partial
    /// failure is exit-only; the enclosing namespace retains attached mounts
    /// until its tasks stop. Detached resources close automatically on failure.
    pub(super) fn install(self, devices: super::CommandDevices) -> io::Result<()> {
        RuntimeFilesystem::Proc.attach(&self.proc)?;
        RuntimeFilesystem::EmptySys.attach(&self.sys)?;
        devices.attach(&self.dev)
    }
}

#[derive(Debug)]
enum RuntimeFilesystem {
    Proc,
    EmptySys,
}
impl RuntimeFilesystem {
    fn attach(self, target: &File) -> io::Result<()> {
        let (name, extra) = match self {
            Self::Proc => ("proc", MountAttrFlags::empty()),
            Self::EmptySys => ("tmpfs", MountAttrFlags::MOUNT_ATTR_RDONLY),
        };
        let context = mount::fsopen(name, FsOpenFlags::FSOPEN_CLOEXEC)?;
        mount::fsconfig_create(&context)?;
        let tree = mount::fsmount(
            &context,
            FsMountFlags::FSMOUNT_CLOEXEC,
            MountAttrFlags::MOUNT_ATTR_NOSUID
                | MountAttrFlags::MOUNT_ATTR_NODEV
                | MountAttrFlags::MOUNT_ATTR_NOEXEC
                | extra,
        )?;
        mount::move_mount(
            &tree,
            "",
            target,
            "",
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )?;
        if matches!(self, Self::Proc) {
            ProcRestrictions::new(tree.into()).install()?;
        }
        Ok(())
    }
}

/// Owns the new proc mount while restricting kernel-created control paths.
/// Commands have not been admitted; these paths cannot be image symlinks.
#[derive(Debug)]
struct ProcRestrictions {
    root: File,
}
impl ProcRestrictions {
    fn new(root: File) -> Self {
        Self { root }
    }

    fn install(self) -> io::Result<()> {
        for path in ["sys", "sysrq-trigger", "irq", "bus"] {
            let target = match rustix::fs::openat2(
                &self.root,
                path,
                OFlags::PATH | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
            ) {
                Ok(fd) => fd,
                // These kernel interfaces are configuration-dependent. An
                // absent interface supplies no authority; all other errors fail.
                Err(rustix::io::Errno::NOENT) => continue,
                Err(error) => return Err(error.into()),
            };
            let view = mount::open_tree(
                &target,
                "",
                OpenTreeFlags::AT_EMPTY_PATH
                    | OpenTreeFlags::OPEN_TREE_CLONE
                    | OpenTreeFlags::OPEN_TREE_CLOEXEC,
            )?;
            mount::mount_setattr(
                &view,
                "",
                MountSetAttrFlags::AT_EMPTY_PATH,
                MountAttr::new(
                    MountAttrFlags::MOUNT_ATTR_RDONLY
                        | MountAttrFlags::MOUNT_ATTR_NOSUID
                        | MountAttrFlags::MOUNT_ATTR_NODEV
                        | MountAttrFlags::MOUNT_ATTR_NOEXEC,
                    MountAttrFlags::empty(),
                    MountPropagationFlags::PRIVATE,
                ),
            )?;
            mount::move_mount(
                &view,
                "",
                &target,
                "",
                MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
    };

    #[test]
    fn absolute_symlinks_resolve_inside_the_image() {
        let image = tempfile::tempdir().unwrap();
        fs::create_dir(image.path().join("dev")).unwrap();
        fs::create_dir(image.path().join("inside")).unwrap();
        fs::create_dir(image.path().join("sys")).unwrap();
        symlink("/inside", image.path().join("proc")).unwrap();
        let targets = RuntimeTargets::resolve(&File::open(image.path()).unwrap()).unwrap();
        assert_eq!(
            targets.proc.metadata().unwrap().ino(),
            fs::metadata(image.path().join("inside")).unwrap().ino()
        );
    }

    #[test]
    fn outside_sibling_is_not_a_runtime_target() {
        let parent = tempfile::tempdir().unwrap();
        fs::create_dir(parent.path().join("image")).unwrap();
        fs::create_dir(parent.path().join("image/dev")).unwrap();
        fs::create_dir(parent.path().join("outside")).unwrap();
        fs::create_dir(parent.path().join("image/sys")).unwrap();
        symlink("../outside", parent.path().join("image/proc")).unwrap();
        assert!(
            RuntimeTargets::resolve(&File::open(parent.path().join("image")).unwrap()).is_err()
        );
    }

    #[test]
    fn root_and_duplicate_target_aliases_are_rejected() {
        let image = tempfile::tempdir().unwrap();
        fs::create_dir(image.path().join("dev")).unwrap();
        fs::create_dir(image.path().join("sys")).unwrap();
        symlink("/", image.path().join("proc")).unwrap();
        let root = File::open(image.path()).unwrap();
        assert!(RuntimeTargets::resolve(&root).is_err());
        fs::remove_file(image.path().join("proc")).unwrap();
        symlink("/sys", image.path().join("proc")).unwrap();
        assert!(RuntimeTargets::resolve(&root).is_err());
    }

    #[test]
    fn missing_runtime_directories_are_created_on_the_image() {
        let image = tempfile::tempdir().unwrap();
        let root = File::open(image.path()).unwrap();
        let root_ino = root.metadata().unwrap().ino();
        RuntimeTargets::resolve(&root).unwrap();
        for name in ["dev", "proc", "sys"] {
            let metadata = fs::metadata(image.path().join(name)).unwrap();
            assert!(metadata.is_dir(), "{name}");
            assert_ne!(metadata.ino(), root_ino, "{name}");
            assert_eq!(metadata.permissions().mode() & 0o777, 0o755, "{name}");
        }
    }

    #[test]
    fn existing_runtime_directories_keep_their_inodes() {
        let image = tempfile::tempdir().unwrap();
        for name in ["dev", "proc", "sys"] {
            fs::create_dir(image.path().join(name)).unwrap();
        }
        let before: Vec<_> = ["dev", "proc", "sys"]
            .into_iter()
            .map(|name| fs::metadata(image.path().join(name)).unwrap().ino())
            .collect();
        RuntimeTargets::resolve(&File::open(image.path()).unwrap()).unwrap();
        let after: Vec<_> = ["dev", "proc", "sys"]
            .into_iter()
            .map(|name| fs::metadata(image.path().join(name)).unwrap().ino())
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn dangling_runtime_symlink_is_not_replaced() {
        let image = tempfile::tempdir().unwrap();
        fs::create_dir(image.path().join("proc")).unwrap();
        fs::create_dir(image.path().join("sys")).unwrap();
        symlink("missing", image.path().join("dev")).unwrap();
        let error = RuntimeTargets::resolve(&File::open(image.path()).unwrap()).unwrap_err();
        assert!(
            error.to_string().contains("command runtime target /dev"),
            "{error}"
        );
        assert!(
            fs::symlink_metadata(image.path().join("dev"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn nested_targets_cannot_hide_each_others_mounts() {
        let image = tempfile::tempdir().unwrap();
        fs::create_dir(image.path().join("dev")).unwrap();
        fs::create_dir_all(image.path().join("proc/inside")).unwrap();
        symlink("/proc/inside", image.path().join("sys")).unwrap();
        assert!(RuntimeTargets::resolve(&File::open(image.path()).unwrap()).is_err());
    }
}
