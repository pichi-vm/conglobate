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
        // Detached views have no attached unmount to make busy. Open readers
        // retain their read-only view after backing names are removed.
        readonly.close()?;
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        assert_eq!(
            file.set_permissions(fs::Permissions::from_mode(0o600))
                .unwrap_err()
                .raw_os_error(),
            Some(30)
        );
        drop(file);
        let readonly = context.materialize(parent)?.read_only()?;
        drop(readonly);
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        let staged = context.materialize(parent)?;
        let limit = DescriptorLimit::exhaust()?;
        let failure = staged.read_only();
        drop(limit);
        let failure = failure.unwrap_err();
        assert_eq!(failure.source_error().raw_os_error(), Some(24));
        assert_eq!(fs::read_dir(parent)?.count(), 1);
        failure.cleanup()?;
        assert_eq!(fs::read_dir(parent)?.count(), 0);
        println!("PRODUCTION_CONTEXT_PARTIAL_SETUP_PASS");
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
#[derive(Debug)]
struct DescriptorLimit(rustix::process::Rlimit);
impl DescriptorLimit {
    fn exhaust() -> io::Result<Self> {
        use rustix::process::{Resource, getrlimit, setrlimit};
        let saved = getrlimit(Resource::Nofile);
        setrlimit(
            Resource::Nofile,
            rustix::process::Rlimit {
                current: Some(0),
                maximum: saved.maximum,
            },
        )?;
        Ok(Self(saved))
    }
}
impl Drop for DescriptorLimit {
    fn drop(&mut self) {
        rustix::process::setrlimit(rustix::process::Resource::Nofile, self.0)
            .expect("restore fixture descriptor limit");
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
