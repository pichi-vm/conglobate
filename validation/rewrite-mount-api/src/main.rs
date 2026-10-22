// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]

use conglobate_mount_api_probe::IdmapRequest;
use rustix::{
    fd::{AsFd, OwnedFd},
    fs::{CWD, Gid, Uid},
    mount::{MoveMountFlags, OpenTreeFlags, UnmountFlags},
};
use std::{fs, io, os::unix::fs::MetadataExt, path::Path};

#[derive(Debug)]
struct Detached(OwnedFd);
impl Detached {
    fn clone_origin() -> io::Result<Self> {
        Ok(Self(rustix::mount::open_tree(
            CWD,
            "/work/origin",
            OpenTreeFlags::OPEN_TREE_CLONE | OpenTreeFlags::OPEN_TREE_CLOEXEC,
        )?))
    }
    fn attach(self, destination: &'static str) -> Result<Attached, (io::Error, Self)> {
        match rustix::mount::move_mount(
            &self.0,
            "",
            CWD,
            destination,
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
        ) {
            Ok(()) => Ok(Attached(destination)),
            Err(error) => Err((error.into(), self)),
        }
    }
}
#[derive(Debug)]
struct Attached(&'static str);
impl Attached {
    fn close(self) -> io::Result<()> {
        rustix::mount::unmount(self.0, UnmountFlags::empty())?;
        // The fixture Drop fallback tolerates an already unmounted target.
        Ok(())
    }
}
impl Drop for Attached {
    fn drop(&mut self) {
        let _ = rustix::mount::unmount(self.0, UnmountFlags::DETACH);
    }
}
#[derive(Debug)]
struct Probe;
impl Probe {
    fn run(&self) -> io::Result<()> {
        if !Path::new("/conglobate-disposable-validation-vm").exists() {
            return Err(io::Error::other("disposable VM fixture required"));
        }
        let pid = fs::read_to_string("/work/userns-pid")?;
        let namespace = fs::File::open(format!("/proc/{}/ns/user", pid.trim()))?;
        fs::write("/work/origin/file", b"fixture")?;
        let detached = Detached::clone_origin()?;
        IdmapRequest::new(namespace.as_fd()).apply(&detached.0)?;
        let (error, detached) = detached.attach("/work/missing/target").unwrap_err();
        assert_eq!(error.raw_os_error(), Some(2));
        let attached = detached
            .attach("/work/mapped")
            .map_err(|(error, _)| error)?;
        let metadata = fs::metadata("/work/mapped/file")?;
        assert_eq!((metadata.uid(), metadata.gid()), (100000, 100000));
        rustix::fs::chown(
            "/work/mapped/file",
            Some(Uid::from_raw(100123)),
            Some(Gid::from_raw(100456)),
        )?;
        let metadata = fs::metadata("/work/origin/file")?;
        assert_eq!((metadata.uid(), metadata.gid()), (123, 456));
        attached.close()?;
        assert_eq!(fs::read_dir("/work/mapped")?.count(), 0);

        // A real file descriptor of the wrong kind must not become an idmap.
        let regular = fs::File::open("/work/origin/file")?;
        let detached = Detached::clone_origin()?;
        assert!(
            IdmapRequest::new(regular.as_fd())
                .apply(&detached.0)
                .is_err()
        );
        drop(detached);

        let detached = Detached::clone_origin()?;
        IdmapRequest::new(namespace.as_fd()).apply(&detached.0)?;
        drop(
            detached
                .attach("/work/mapped")
                .map_err(|(error, _)| error)?,
        );
        assert_eq!(fs::read_dir("/work/mapped")?.count(), 0);
        println!("IDMAP_TRANSLATION_AND_CLEANUP_PASS");
        Ok(())
    }
}
fn main() -> io::Result<()> {
    Probe.run()
}
