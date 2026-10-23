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
    marker::PhantomData,
    os::fd::AsRawFd,
};

/// Private named source on the build workspace. Linux permits cloning an
/// anonymous file mount but rejects attaching it with move_mount (ENOENT).
/// Retain this name while creating/attaching views; it is never image-visible.
/// There is no writable descriptor after construction. Drop removes the name;
/// retained views keep their inode alive until their namespace is destroyed.
#[derive(Debug)]
pub struct ResolverSource {
    // Remove the name while the proc-fd parent path still exists.
    path: tempfile::TempPath,
    file: File,
    _parent: File,
}
impl ResolverSource {
    /// Trusted setup transport only. The enclosing step retains this source
    /// until the receiving helper and all descendants have stopped.
    pub(crate) fn view(&self) -> io::Result<ResolverView> {
        ResolverView::from_handle(self.mount()?.tree)
    }
    pub fn prepare(input: &ResolverInput, workspace: &Workspace) -> io::Result<Self> {
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
    pub fn mount(&self) -> io::Result<ResolverMount<'_>> {
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
        Ok(ResolverMount {
            tree: tree.into(),
            _source: PhantomData,
        })
    }
    /// Explicit cleanup retains its owner on failure. Existing attached views
    /// keep only the read-only inode, with no source pathname exposed to them.
    pub fn close(self) -> Result<(), ResolverSourceFailure> {
        let result = fs::unlinkat(
            &self._parent,
            self.path.file_name().expect("generated temporary filename"),
            fs::AtFlags::empty(),
        );
        match result {
            Ok(()) => Ok(()),
            Err(error) => Err(ResolverSourceFailure {
                error: error.into(),
                owner: self,
            }),
        }
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
    pub(crate) fn handle(&self) -> &File {
        &self.tree
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
#[derive(Debug, thiserror::Error)]
#[error("resolver source cleanup: {error}")]
pub struct ResolverSourceFailure {
    error: io::Error,
    owner: ResolverSource,
}
impl ResolverSourceFailure {
    pub fn cleanup(self) -> Result<(), Self> {
        self.owner.close()
    }
}

/// A detached mount, already read-only before it can be attached. Borrowing the
/// source ensures its name survives move_mount. No writable descriptor escapes.
#[derive(Debug)]
pub struct ResolverMount<'source> {
    tree: File,
    _source: PhantomData<&'source ResolverSource>,
}
impl ResolverMount<'_> {
    /// Dedicated helper only, before command admission. The target must have
    /// been selected in this helper's current private mount namespace. The
    /// caller retains its scaffold owner until commands and mounts are gone.
    /// Any setup failure is exit-only; namespace disposal removes the mount.
    pub fn attach(self, target: &ResolverTarget) -> io::Result<()> {
        mount::move_mount(
            &self.tree,
            "",
            target.mount_target()?,
            "",
            mount::MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH
                | mount::MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )?;
        Ok(())
    }
}
