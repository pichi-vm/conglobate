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
    Probe {
        root: PathBuf::from("/tmp/context-mount-probe"),
    }
    .run()
}
