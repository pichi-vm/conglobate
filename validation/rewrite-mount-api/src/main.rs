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
        use conglobate::isolation::PrivateMountNamespace;
        let original_namespace = fs::metadata("/proc/thread-self/ns/mnt")?.ino();
        let ready = std::sync::Arc::new(std::sync::Barrier::new(2));
        let release = std::sync::Arc::new(std::sync::Barrier::new(2));
        let child_ready = ready.clone();
        let child_release = release.clone();
        let thread = std::thread::spawn(move || {
            child_ready.wait();
            child_release.wait();
        });
        ready.wait();
        let rejected = PrivateMountNamespace::enter();
        release.wait();
        thread.join().unwrap();
        assert!(
            matches!(rejected, Err(conglobate::isolation::NamespaceFailure::Before(error)) if error.to_string().contains("single-threaded"))
        );
        assert_eq!(
            fs::metadata("/proc/thread-self/ns/mnt")?.ino(),
            original_namespace
        );
        let scope = PrivateMountNamespace::enter().map_err(|e| io::Error::other(e.to_string()))?;
        assert_ne!(
            fs::metadata("/proc/thread-self/ns/mnt")?.ino(),
            original_namespace
        );
        let pid = fs::read_to_string("/work/userns-pid")?;
        let namespace = fs::File::open(format!("/proc/{}/ns/user", pid.trim()))?;
        fs::write("/work/origin/file", b"fixture")?;
        self.production(&namespace, &scope)?;
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
        scope
            .restore()
            .map_err(|e| io::Error::other(e.to_string()))?;
        assert_eq!(
            fs::metadata("/proc/thread-self/ns/mnt")?.ino(),
            original_namespace
        );
        let scope = PrivateMountNamespace::enter().map_err(|e| io::Error::other(e.to_string()))?;
        drop(scope);
        assert_eq!(
            fs::metadata("/proc/thread-self/ns/mnt")?.ino(),
            original_namespace
        );
        println!("PRODUCTION_PRIVATE_NAMESPACE_PASS");
        Ok(())
    }

    fn production(
        &self,
        namespace: &fs::File,
        scope: &conglobate::isolation::PrivateMountNamespace,
    ) -> io::Result<()> {
        use conglobate::filesystem::{Detached, Mount};
        let origin = fs::File::open("/work/origin")?;
        let mount = Mount::<Detached>::clone_from(&origin, scope)?;
        drop(origin);
        let invalid = fs::File::open("/work/origin/file")?;
        let failure = mount.idmap(invalid).unwrap_err();
        assert_eq!(failure.source_error().raw_os_error(), Some(22));
        let (_, mount, invalid) = failure.into_parts();
        drop(invalid);
        let mapped = mount
            .idmap(namespace.try_clone()?)
            .map_err(|failure| io::Error::other(failure.to_string()))?;
        let metadata = mapped.root_metadata()?;
        assert_eq!((metadata.uid(), metadata.gid()), (100000, 100000));
        use conglobate::filesystem::{MountPoint, UnmountFailure};
        let parent = Path::new("/work/points");
        let moved = Path::new("/work/points-renamed");
        fs::create_dir(parent)?;
        let point = MountPoint::create(parent)?;
        let path = fs::read_dir(parent)?.next().unwrap()?.path();
        // Benign fixture removes the empty target to force move_mount failure.
        fs::remove_dir(&path)?;
        let failure = mapped.attach(point).unwrap_err();
        let (error, mapped, point) = failure.into_parts();
        assert_eq!(error.raw_os_error(), Some(2));
        fs::create_dir(&path)?;
        let attached = mapped
            .attach(point)
            .map_err(|e| io::Error::other(e.to_string()))?;
        assert_eq!(attached.root_metadata()?.uid(), 100000);
        assert_eq!(
            fs::read_dir(format!("/proc/1/root{}", path.display()))?.count(),
            0
        );
        let busy = fs::File::open(path.join("file"))?;
        fs::rename(parent, moved)?;
        let failure = attached.close().unwrap_err();
        assert!(
            matches!(&failure, UnmountFailure::Mounted { source, .. } if source.raw_os_error() == Some(16))
        );
        assert_eq!(fs::read_dir(moved)?.count(), 1);
        drop(busy);
        failure
            .cleanup()
            .map_err(|e| io::Error::other(e.to_string()))?;
        assert_eq!(fs::read_dir(moved)?.count(), 0);
        let origin = fs::File::open("/work/origin")?;
        let mapped = Mount::<Detached>::clone_from(&origin, scope)?
            .idmap(namespace.try_clone()?)
            .map_err(|e| io::Error::other(e.to_string()))?;
        drop(origin);
        let attached = mapped
            .attach(MountPoint::create(moved)?)
            .map_err(|e| io::Error::other(e.to_string()))?;
        drop(attached);
        assert_eq!(fs::read_dir(moved)?.count(), 0);
        assert_eq!(fs::read("/work/origin/file")?, b"fixture");
        println!("PRODUCTION_ATTACHMENT_LIFECYCLE_PASS");
        // Detached owners must not leave an extra busy mount after dropping.
        rustix::mount::unmount("/work/origin", UnmountFlags::empty())?;
        // Reattach the fixture through its existing loop device, found by the
        // fixture mount command. No raw image conversion is involved here.
        let status = std::process::Command::new("mount")
            .args(["-o", "loop", "/work/volume.ext4", "/work/origin"])
            .status()?;
        assert!(status.success());
        println!("PRODUCTION_DETACHED_MOUNT_PASS");
        Ok(())
    }
}
fn main() -> io::Result<()> {
    Probe.run()
}
