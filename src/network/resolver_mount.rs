// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use super::{ResolverInput, ResolverTarget};
use crate::storage::Workspace;
use rustix::{
    fs::{self, Mode, OFlags},
    mount::{
        self, MountAttr, MountAttrFlags, MountPropagationFlags, MountSetAttrFlags, OpenTreeFlags,
    },
};
use std::{
    fs::File,
    io::{self, Write},
    os::fd::AsRawFd,
};

/// Private named source on the build workspace. Linux permits cloning an
/// anonymous file mount but rejects attaching it with move_mount (ENOENT).
/// Retain this name while creating/attaching views; it is never image-visible.
/// There is no writable descriptor after construction. Drop removes the name;
/// retained views keep their inode alive until their namespace is destroyed.
#[derive(Debug)]
pub(crate) struct ResolverSource {
    // Remove the name while the proc-fd parent path still exists.
    path: tempfile::TempPath,
    file: File,
    _parent: File,
}
impl ResolverSource {
    /// Trusted setup transport only. The enclosing step retains this source
    /// until the receiving helper and all descendants have stopped.
    pub(crate) fn view(&self) -> io::Result<ResolverView> {
        ResolverView::from_handle(self.mount()?)
    }
    pub(crate) fn prepare(input: &ResolverInput, workspace: &Workspace) -> io::Result<Self> {
        let parent = workspace.staging_parent()?;
        let directory = format!("/proc/self/fd/{}", parent.as_raw_fd());
        let mut backing = tempfile::Builder::new()
            .prefix("resolver-")
            .tempfile_in(directory)?;
        backing.write_all(input.bytes())?;
        fs::fchmod(backing.as_file(), Mode::from_raw_mode(0o444))?;
        let file = File::from(fs::open(
            backing.path(),
            OFlags::PATH | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        let path = backing.into_temp_path();
        Ok(Self {
            path,
            file,
            _parent: parent,
        })
    }
    pub(crate) fn mount(&self) -> io::Result<File> {
        let tree = mount::open_tree(
            &self.file,
            "",
            OpenTreeFlags::AT_EMPTY_PATH
                | OpenTreeFlags::OPEN_TREE_CLONE
                | OpenTreeFlags::OPEN_TREE_CLOEXEC,
        )?;
        mount::mount_setattr(
            &tree,
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
        Ok(tree.into())
    }
    /// Existing attached views retain their read-only inode after unlinking.
    pub(crate) fn close(self) -> io::Result<()> {
        fs::unlinkat(
            &self._parent,
            self.path.file_name().expect("generated temporary filename"),
            fs::AtFlags::empty(),
        )?;
        Ok(())
    }
}

/// Received detached view. No source pathname or writable file handle travels
/// with it. Sender-side source ownership remains with the enclosing build.
#[derive(Debug)]
pub(crate) struct ResolverView {
    tree: File,
}
impl ResolverView {
    pub(crate) fn from_handle(tree: File) -> io::Result<Self> {
        let required = fs::StatVfsMountFlags::RDONLY
            | fs::StatVfsMountFlags::NOSUID
            | fs::StatVfsMountFlags::NODEV
            | fs::StatVfsMountFlags::NOEXEC;
        if !tree.metadata()?.is_file()
            || fs::fstatfs(&tree)?.f_type != 0x0102_1994
            || !fs::fstatvfs(&tree)?.f_flag.contains(required)
        {
            return Err(io::Error::other(
                "resolver input requires a restricted read-only tmpfs file mount",
            ));
        }
        Ok(Self { tree })
    }
    pub(crate) fn into_handle(self) -> File {
        self.tree
    }
    pub(crate) fn install(self, root: File) -> io::Result<()> {
        let target = ResolverTarget::existing(root).map_err(io::Error::other)?;
        mount::move_mount(
            &self.tree,
            "",
            target.mount_target()?,
            "",
            mount::MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH
                | mount::MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )?;
        // existing() cannot create scaffolding; these handles can all close
        // before dropping initial-user-namespace privilege.
        target.release().map_err(io::Error::other)
    }
}
#[cfg(test)]
mod tests {
    use super::ResolverSource;
    use std::fs;

    #[test]
    fn resolver_cleanup_preserves_io_error_and_does_not_remove_a_directory() {
        let parent = tempfile::tempdir().unwrap();
        let temporary = tempfile::NamedTempFile::new_in(parent.path()).unwrap();
        let (file, path) = temporary.into_parts();
        let original = path.to_path_buf();
        let saved = parent.path().join("saved");
        fs::rename(&original, &saved).unwrap();
        fs::create_dir(&original).unwrap();
        let source = ResolverSource {
            path,
            file,
            _parent: fs::File::open(parent.path()).unwrap(),
        };
        let error = source.close().unwrap_err();
        assert_eq!(
            error.raw_os_error(),
            Some(rustix::io::Errno::ISDIR.raw_os_error())
        );
        assert!(original.is_dir());
        assert!(saved.is_file());
    }
}
