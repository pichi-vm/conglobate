// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]

//! Safe API qualification. Runtime calls belong only in the disposable VM probe.
use rustix::{
    fd::{AsFd, BorrowedFd},
    mount::{
        MountAttr, MountAttrFlags, MountAttrParamFlags, MountPropagationFlags, MountSetAttrFlags,
    },
};

/// Scoped request cannot retain a closed user namespace descriptor.
///
/// ```compile_fail
/// use conglobate_mount_api_probe::IdmapRequest;
/// use std::{fs::File, os::fd::AsFd};
/// let request = {
///     let namespace = File::open("/proc/self/ns/user").unwrap();
///     IdmapRequest::new(namespace.as_fd())
/// };
/// drop(request);
/// ```
pub struct IdmapRequest<'a> {
    attributes: MountAttr<'a>,
}
impl<'a> IdmapRequest<'a> {
    pub fn new(namespace: BorrowedFd<'a>) -> Self {
        Self {
            attributes: MountAttr::new(
                MountAttrFlags::MOUNT_ATTR_NOSUID | MountAttrFlags::MOUNT_ATTR_NODEV,
                MountAttrFlags::empty(),
                MountPropagationFlags::empty(),
            )
            .set_param_flag(MountAttrParamFlags::IdMap(namespace)),
        }
    }

    /// Requires a detached mount owned by the caller. Runtime qualification of
    /// idmapping and owner translation must run only in a disposable VM.
    pub fn apply(self, mount: impl AsFd) -> rustix::io::Result<()> {
        rustix::mount::mount_setattr(mount, "", MountSetAttrFlags::AT_EMPTY_PATH, self.attributes)
    }
}
