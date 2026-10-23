// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Owned detached mount preparation. Command admission and namespace creation
//! are separate gates; callers must operate in the private builder namespace.
use rustix::{
    fd::AsFd,
    mount::{
        MountAttr, MountAttrFlags, MountAttrParamFlags, MountPropagationFlags, MountSetAttrFlags,
        OpenTreeFlags,
    },
};
use std::{fs::File, io, marker::PhantomData, rc::Rc};
mod attachment;
pub use attachment::{
    Attached, AttachedResources, AttachmentFailure, InUse, InUseResources, MountPoint,
    RootLaunchFailure, RootWaitFailure, UnmountFailure,
};

mod private {
    pub trait Sealed {}
}
pub trait MountState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub struct Detached;
#[derive(Debug)]
pub struct Idmapped;
impl private::Sealed for Detached {}
impl private::Sealed for Idmapped {}
impl MountState for Detached {
    type Resources = DetachedResources;
}
impl MountState for Idmapped {
    type Resources = IdmappedResources;
}
#[derive(Debug)]
pub struct DetachedResources {
    root: File,
}
#[derive(Debug)]
pub struct IdmappedResources {
    root: File,
    // Retain explicit identity ownership independently of kernel idmap references.
    _namespace: File,
    output_namespace: Option<File>,
}

/// Owns a detached mount tree. Closing its descriptor releases it without a
/// pathname-based unmount. No descriptor or writable root escapes this owner.
/// It is thread-affine to preserve the future namespace attachment boundary.
///
/// ```compile_fail
/// use conglobate::filesystem::{Mount, Idmapped};
/// use std::fs::File;
/// fn cannot_remap(mount: Mount<Idmapped>, namespace: File) {
///     mount.idmap(namespace);
/// }
/// ```
#[derive(Debug)]
pub struct Mount<'ns, S: MountState> {
    resources: S::Resources,
    _scope: &'ns crate::isolation::PrivateMountNamespace,
    _thread: PhantomData<Rc<()>>,
}

/// A failed idmap attempt retains the detached mount and supplied namespace.
/// No mapped-state assertion is returned on failure. Dropping the error closes
/// both descriptors, releasing the detached tree without an attached mount.
#[derive(Debug, thiserror::Error)]
#[error("idmapping detached mount: {source}")]
pub struct IdmapFailure<'ns> {
    source: io::Error,
    mount: Mount<'ns, Detached>,
    namespace: File,
}
impl<'ns> IdmapFailure<'ns> {
    pub fn source_error(&self) -> &io::Error {
        &self.source
    }
    pub fn into_parts(self) -> (io::Error, Mount<'ns, Detached>, File) {
        (self.source, self.mount, self.namespace)
    }
}
impl<'ns> Mount<'ns, Detached> {
    pub(crate) fn from_detached(
        root: File,
        scope: &'ns crate::isolation::PrivateMountNamespace,
    ) -> io::Result<Self> {
        scope.verify_current()?;
        Ok(Self {
            resources: DetachedResources { root },
            _scope: scope,
            _thread: PhantomData,
        })
    }
    /// Clone only this filesystem view, never recursively include submounts.
    /// The source descriptor pins the selected root during acquisition. The
    /// detached result independently pins its superblock after source closure.
    pub fn clone_from(
        root: &File,
        scope: &'ns crate::isolation::PrivateMountNamespace,
    ) -> io::Result<Self> {
        scope.verify_current()?;
        let tree = rustix::mount::open_tree(
            root,
            "",
            OpenTreeFlags::AT_EMPTY_PATH
                | OpenTreeFlags::OPEN_TREE_CLONE
                | OpenTreeFlags::OPEN_TREE_CLOEXEC,
        )?;
        Ok(Self {
            resources: DetachedResources { root: tree.into() },
            _scope: scope,
            _thread: PhantomData,
        })
    }

    /// Map filesystem IDs through the supplied user namespace, keeping device
    /// access and set-ID/file-capability elevation disabled on the view.
    pub fn idmap(self, namespace: File) -> Result<Mount<'ns, Idmapped>, IdmapFailure<'ns>> {
        let attributes = MountAttr::new(
            MountAttrFlags::MOUNT_ATTR_NOSUID | MountAttrFlags::MOUNT_ATTR_NODEV,
            MountAttrFlags::empty(),
            MountPropagationFlags::PRIVATE,
        )
        .set_param_flag(MountAttrParamFlags::IdMap(namespace.as_fd()));
        match rustix::mount::mount_setattr(
            &self.resources.root,
            "",
            MountSetAttrFlags::AT_EMPTY_PATH,
            attributes,
        ) {
            Ok(()) => Ok(Mount {
                resources: IdmappedResources {
                    root: self.resources.root,
                    _namespace: namespace,
                    output_namespace: None,
                },
                _scope: self._scope,
                _thread: PhantomData,
            }),
            Err(source) => Err(IdmapFailure {
                source: source.into(),
                mount: self,
                namespace,
            }),
        }
    }
}
impl Mount<'_, Idmapped> {
    /// Metadata of the selected root in the mapped view; no path traversal or
    /// raw descriptor escape. This is not proof of command sandbox isolation.
    pub fn root_metadata(&self) -> io::Result<std::fs::Metadata> {
        self.resources.root.metadata()
    }
}

impl<'ns> Mount<'ns, Idmapped> {
    /// Build the detached bootstrap hierarchy before attachment/admission. The
    /// fixed mountpoint must be an ordinary directory; image symlinks cannot
    /// redirect the supervisor's later unmount. Failure drops detached views.
    pub(crate) fn with_output(self, output: Self) -> io::Result<Self> {
        use rustix::fs::{AtFlags, Mode, OFlags, ResolveFlags};
        use std::os::unix::fs::MetadataExt;
        if self.resources.output_namespace.is_some() || output.resources.output_namespace.is_some()
        {
            return Err(io::Error::other(
                "nested bootstrap output views are unsupported",
            ));
        }
        self._scope.verify_current()?;
        output._scope.verify_current()?;
        let left = self.resources._namespace.metadata()?;
        let right = output.resources._namespace.metadata()?;
        if (left.dev(), left.ino()) != (right.dev(), right.ino()) {
            return Err(io::Error::other(
                "bootstrap views require the same user namespace",
            ));
        }
        match rustix::fs::mkdirat(&self.resources.root, "mnt", Mode::from_raw_mode(0o755)) {
            Ok(()) => {
                let owner = self.resources.root.metadata()?;
                rustix::fs::chownat(
                    &self.resources.root,
                    "mnt",
                    Some(rustix::process::Uid::from_raw(owner.uid())),
                    Some(rustix::process::Gid::from_raw(owner.gid())),
                    AtFlags::SYMLINK_NOFOLLOW,
                )?;
            }
            Err(rustix::io::Errno::EXIST) => {}
            Err(error) => return Err(error.into()),
        }
        let target = rustix::fs::openat2(
            &self.resources.root,
            "mnt",
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )?;
        rustix::mount::move_mount(
            &output.resources.root,
            "",
            &target,
            "",
            rustix::mount::MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH
                | rustix::mount::MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )?;
        let IdmappedResources {
            root,
            _namespace,
            output_namespace: _,
        } = output.resources;
        drop(root);
        Ok(Self {
            resources: IdmappedResources {
                output_namespace: Some(_namespace),
                ..self.resources
            },
            _scope: self._scope,
            _thread: PhantomData,
        })
    }
}
