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
pub struct Mount<S: MountState> {
    resources: S::Resources,
    _thread: PhantomData<Rc<()>>,
}

/// A failed idmap attempt retains the detached mount and supplied namespace.
/// No mapped-state assertion is returned on failure. Dropping the error closes
/// both descriptors, releasing the detached tree without an attached mount.
#[derive(Debug, thiserror::Error)]
#[error("idmapping detached mount: {source}")]
pub struct IdmapFailure {
    source: io::Error,
    mount: Mount<Detached>,
    namespace: File,
}
impl IdmapFailure {
    pub fn source_error(&self) -> &io::Error {
        &self.source
    }
    pub fn into_parts(self) -> (io::Error, Mount<Detached>, File) {
        (self.source, self.mount, self.namespace)
    }
}
impl Mount<Detached> {
    /// Clone only this filesystem view, never recursively include submounts.
    /// The source descriptor pins the selected root during acquisition. The
    /// detached result independently pins its superblock after source closure.
    pub fn clone_from(root: &File) -> io::Result<Self> {
        let tree = rustix::mount::open_tree(
            root,
            "",
            OpenTreeFlags::AT_EMPTY_PATH
                | OpenTreeFlags::OPEN_TREE_CLONE
                | OpenTreeFlags::OPEN_TREE_CLOEXEC,
        )?;
        Ok(Self {
            resources: DetachedResources { root: tree.into() },
            _thread: PhantomData,
        })
    }

    /// Map filesystem IDs through the supplied user namespace, keeping device
    /// access and set-ID/file-capability elevation disabled on the view.
    pub fn idmap(self, namespace: File) -> Result<Mount<Idmapped>, IdmapFailure> {
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
                },
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
impl Mount<Idmapped> {
    /// Metadata of the selected root in the mapped view; no path traversal or
    /// raw descriptor escape. This is not proof of command sandbox isolation.
    pub fn root_metadata(&self) -> io::Result<std::fs::Metadata> {
        self.resources.root.metadata()
    }
}
