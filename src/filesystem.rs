// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Owned detached mount preparation. Command admission and namespace creation
//! are separate gates; callers must operate in the private builder namespace.
use rustix::{
    fd::AsFd,
    mount::{
        MountAttr, MountAttrFlags, MountAttrParamFlags, MountPropagationFlags, MountSetAttrFlags,
    },
};
use std::{fs::File, io, marker::PhantomData, rc::Rc};
mod attachment;
mod children;
pub(crate) use attachment::{Attached, InUse, MountPoint, RootWaitFailure};

mod private {
    pub(crate) trait Sealed {}
}
pub(crate) trait MountState: private::Sealed + std::fmt::Debug {
    type Resources: std::fmt::Debug;
}
#[derive(Debug)]
pub(crate) struct Detached;
#[derive(Debug)]
pub(crate) struct Idmapped;
impl private::Sealed for Detached {}
impl private::Sealed for Idmapped {}
impl MountState for Detached {
    type Resources = DetachedResources;
}
impl MountState for Idmapped {
    type Resources = IdmappedResources;
}
#[derive(Debug)]
pub(crate) struct DetachedResources {
    root: File,
}
#[derive(Debug)]
pub(crate) struct IdmappedResources {
    root: File,
    // Retain explicit identity ownership independently of kernel idmap references.
    _namespace: File,
    output: Option<OutputView>,
}

#[derive(Debug)]
struct OutputView {
    root: File,
    namespace: File,
}

/// Owns a detached mount tree. Closing its descriptor releases it without a
/// pathname-based unmount. No descriptor or writable root escapes this owner.
/// It is thread-affine to preserve the future namespace attachment boundary.
///
#[derive(Debug)]
pub(crate) struct Mount<'ns, S: MountState> {
    resources: S::Resources,
    _scope: &'ns crate::isolation::PrivateMountNamespace,
    _thread: PhantomData<Rc<()>>,
}

/// A failed idmap attempt retains the detached mount and supplied namespace.
/// No mapped-state assertion is returned on failure. Dropping the error closes
/// both descriptors, releasing the detached tree without an attached mount.
#[derive(Debug, thiserror::Error)]
#[error("idmapping detached mount: {source}")]
pub(crate) struct IdmapFailure<'ns> {
    source: io::Error,
    mount: Mount<'ns, Detached>,
    namespace: File,
}
impl<'ns> IdmapFailure<'ns> {
    pub(crate) fn into_parts(self) -> (io::Error, Mount<'ns, Detached>, File) {
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

    /// Map filesystem IDs through the supplied user namespace, keeping device
    /// access and set-ID/file-capability elevation disabled on the view.
    pub(crate) fn idmap(self, namespace: File) -> Result<Mount<'ns, Idmapped>, IdmapFailure<'ns>> {
        let attributes = MountAttr::new(
            MountAttrFlags::MOUNT_ATTR_NOSUID | MountAttrFlags::MOUNT_ATTR_NODEV,
            MountAttrFlags::empty(),
            MountPropagationFlags::PRIVATE,
        )
        .set_param_flag(MountAttrParamFlags::IdMap(namespace.as_fd()));
        match rustix::mount::mount_setattr(
            &self.resources.root,
            "",
            MountSetAttrFlags::AT_EMPTY_PATH | MountSetAttrFlags::AT_RECURSIVE,
            attributes,
        ) {
            Ok(()) => Ok(Mount {
                resources: IdmappedResources {
                    root: self.resources.root,
                    _namespace: namespace,
                    output: None,
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
impl Mount<'_, Idmapped> {}

impl<'ns> Mount<'ns, Idmapped> {
    /// Retain both detached bootstrap views before attachment/admission. The
    /// fixed mountpoint must be an ordinary directory; image symlinks cannot
    /// redirect the supervisor's later unmount. Failure drops detached views.
    pub(crate) fn with_output(self, output: Self) -> io::Result<Self> {
        use rustix::fs::{Mode, OFlags, ResolveFlags};
        use std::os::unix::fs::MetadataExt;
        if self.resources.output.is_some() || output.resources.output.is_some() {
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
        let target = rustix::fs::openat2(
            &self.resources.root,
            "mnt",
            OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
        )?;
        drop(target);
        let IdmappedResources {
            root,
            _namespace,
            output: _,
        } = output.resources;
        Ok(Self {
            resources: IdmappedResources {
                output: Some(OutputView {
                    root,
                    namespace: _namespace,
                }),
                ..self.resources
            },
            _scope: self._scope,
            _thread: PhantomData,
        })
    }
}
