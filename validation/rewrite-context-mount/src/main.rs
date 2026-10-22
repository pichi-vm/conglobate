// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
#![forbid(unsafe_code)]

use rustix::{
    fs::{Mode, OFlags},
    mount::{MountFlags, UnmountFlags},
};
use std::{
    fs::{self, File},
    io,
    path::PathBuf,
};

#[derive(Debug)]
struct Probe {
    root: PathBuf,
}
impl Probe {
    fn production(&self) -> Result<(), Box<dyn std::error::Error>> {
        use conglobate::{
            context::CapturedContext,
            path::{ContextDirectory, ContextPath},
            request::CommitIdentity,
        };
        use std::{os::unix::fs::PermissionsExt, path::Path};
        let commit = fs::read_to_string("/fixture-commit")?;
        let context = CapturedContext::capture(
            Path::new("/fixture"),
            &CommitIdentity::try_from(commit.trim())?,
            &ContextDirectory::try_from(Path::new("."))?,
        )?;
        let parent = Path::new("/work");
        let readonly = context.materialize(parent)?.read_only()?;
        let file = readonly.open(&ContextPath::try_from(Path::new("file"))?)?;
        let error = file
            .set_permissions(fs::Permissions::from_mode(0o600))
            .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(30));
        // An open file keeps a normal unmount busy. The error must retain cleanup
        // authority until that file is closed, without deleting the mounted tree.
        let failure = readonly.close().unwrap_err();
        assert!(matches!(
            failure,
            conglobate::context::ContextMountFailure::Mounted { .. }
        ));
        assert_eq!(fs::read_dir(parent)?.count(), 1);
        drop(file);
        failure.cleanup()?;
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        let readonly = context.materialize(parent)?.read_only()?;
        drop(readonly);
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("PRODUCTION_CONTEXT_LIFECYCLE_PASS");
        Ok(())
    }
    fn run(&self) -> io::Result<()> {
        fs::create_dir(&self.root)?;
        fs::write(self.root.join("content"), b"committed bytes")?;
        let previous = File::open(&self.root)?;
        rustix::mount::mount_bind(&self.root, &self.root)?;
        rustix::mount::mount_remount(
            &self.root,
            MountFlags::BIND
                | MountFlags::RDONLY
                | MountFlags::NOSUID
                | MountFlags::NODEV
                | MountFlags::NOEXEC,
            "",
        )?;
        let readonly = File::open(&self.root)?;
        let flags = OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL;
        let error =
            rustix::fs::openat(&readonly, "new", flags, Mode::RUSR | Mode::WUSR).unwrap_err();
        assert_eq!(error, rustix::io::Errno::ROFS);
        // Benign fixture: establish why the pre-remount descriptor must be closed.
        let old_view = rustix::fs::openat(&previous, "old-view", flags, Mode::RUSR | Mode::WUSR)?;
        drop(old_view);
        drop(previous);
        assert_eq!(fs::read(self.root.join("content"))?, b"committed bytes");
        drop(readonly);
        rustix::mount::unmount(&self.root, UnmountFlags::empty())?;
        fs::remove_dir_all(&self.root)?;
        println!("READONLY_CONTEXT_VIEW_PASS");
        Ok(())
    }
}
fn main() -> io::Result<()> {
    // Deliberately require a fixture marker supplied only in the disposable VM.
    if !std::path::Path::new("/conglobate-disposable-validation-vm").exists() {
        return Err(io::Error::other("disposable validation VM marker required"));
    }
    let probe = Probe {
        root: PathBuf::from("/tmp/context-mount-probe"),
    };
    probe.run()?;
    probe
        .production()
        .map_err(|error| io::Error::other(error.to_string()))
}
