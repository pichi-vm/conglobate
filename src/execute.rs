// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Step workers use fresh mount/PID namespaces and retain real root identity.

use crate::Error;
use clap::Args;
use nix::{
    mount::{MsFlags, mount},
    sched::{CloneFlags, unshare},
    sys::{
        prctl::set_pdeathsig,
        signal::Signal,
        stat::{Mode, SFlag, makedev, mknod},
    },
    unistd::{chdir, chroot},
};
use std::{
    os::fd::AsFd,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

#[derive(Debug, Clone, Args)]
pub struct Step {
    #[arg(long, hide = true)]
    pub expected_parent: Option<u32>,
    #[arg(long)]
    pub root: PathBuf,
    #[arg(long)]
    pub context: PathBuf,
    #[arg(long)]
    pub network: bool,
    #[arg(long)]
    pub carapace: Option<String>,
    #[arg(last = true, required = true)]
    pub command: String,
}

impl Step {
    fn command(&self, executable: &Path, mode: &str) -> Result<Command, Error> {
        let mut command = Command::new(executable);
        command
            .arg(mode)
            .arg("--expected-parent")
            .arg(std::fs::read_link("/proc/self")?)
            .arg("--root")
            .arg(&self.root)
            .arg("--context")
            .arg(&self.context);
        if self.network {
            command.arg("--network");
        }
        if let Some(root) = &self.carapace {
            command.arg("--carapace").arg(root);
        }
        command.arg("--").arg(&self.command);
        Ok(command)
    }

    pub fn run(&self, executable: &Path) -> Result<(), Error> {
        let child = self.command(executable, "__step")?.spawn()?;
        success(wait_child(child, Signal::SIGTERM)?)
    }

    /// Run in a fresh, single-threaded helper, never in the caller's process.
    pub fn supervisor(&self) -> Result<(), Error> {
        die_with_parent(self.expected_parent)?;
        crate::cancel::install()?;
        let mut flags = CloneFlags::CLONE_NEWNS | CloneFlags::CLONE_NEWPID;
        if !self.network {
            flags |= CloneFlags::CLONE_NEWNET;
        }
        unshare(flags)?;
        private_mounts()?;
        let mountpoints = ["context", "dev", "proc", "sys"];
        let created: Vec<_> = mountpoints
            .iter()
            .filter(|name| {
                std::fs::symlink_metadata(self.root.join(name))
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            })
            .copied()
            .collect();
        let child = self.command(&std::env::current_exe()?, "__init")?.spawn()?;
        let status = wait_child(child, Signal::SIGKILL);
        for name in mountpoints {
            match nix::mount::umount2(&self.root.join(name), nix::mount::MntFlags::MNT_DETACH) {
                Ok(()) | Err(nix::errno::Errno::EINVAL | nix::errno::Errno::ENOENT) => {}
                Err(error) => return Err(error.into()),
            }
        }
        for name in created {
            match std::fs::remove_dir(self.root.join(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }

        // Linux completes PID-namespace teardown (SIGKILL to all remaining
        // members) before the namespace init's parent can reap it.
        success(status?)
    }

    /// This helper is PID 1 in the step's PID namespace. Its exit terminates
    /// every descendant, including double-forked and setsid daemons.
    pub fn init(&self) -> Result<(), Error> {
        die_with_parent(self.expected_parent)?;
        if nix::unistd::getpid().as_raw() != 1 {
            return Err(Error::Invalid("step init must run as PID 1".into()));
        }
        let root = self.root.canonicalize()?;
        let context = self.context.canonicalize()?;
        for name in ["context", "dev", "proc", "sys"] {
            mountpoint(&root, name)?;
        }
        mount(
            Some(context.as_path()),
            &root.join("context"),
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )?;
        mount(
            None::<&str>,
            &root.join("context"),
            None::<&str>,
            MsFlags::MS_BIND
                | MsFlags::MS_REMOUNT
                | MsFlags::MS_RDONLY
                | MsFlags::MS_NOSUID
                | MsFlags::MS_NODEV,
            None::<&str>,
        )?;
        // No host /dev or /sys exposure. /proc belongs to this PID namespace.
        mount(
            Some("tmpfs"),
            &root.join("dev"),
            Some("tmpfs"),
            MsFlags::MS_NOSUID,
            Some("mode=755"),
        )?;
        mount(
            Some("tmpfs"),
            &root.join("sys"),
            Some("tmpfs"),
            MsFlags::MS_RDONLY | MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            None::<&str>,
        )?;
        mount(
            Some("proc"),
            &root.join("proc"),
            Some("proc"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
            None::<&str>,
        )?;
        chroot(&root)?;
        chdir("/")?;
        for (name, minor) in [("null", 3), ("zero", 5), ("random", 8), ("urandom", 9)] {
            let path = format!("/dev/{name}");
            mknod(
                Path::new(&path),
                SFlag::S_IFCHR,
                Mode::from_bits_truncate(0o666),
                makedev(1, minor),
            )?;
            // Device permissions are runtime policy, not checkout metadata;
            // the caller's umask must not prevent build users writing /dev/null.
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))?;
        }
        std::fs::create_dir("/dev/shm")?;
        mount(
            Some("tmpfs"),
            Path::new("/dev/shm"),
            Some("tmpfs"),
            MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
            Some("mode=1777"),
        )?;
        std::os::unix::fs::symlink("/proc/self/fd", "/dev/fd")?;
        for (name, fd) in [("stdin", 0), ("stdout", 1), ("stderr", 2)] {
            std::os::unix::fs::symlink(format!("/proc/self/fd/{fd}"), format!("/dev/{name}"))?;
        }
        let mut command = Command::new("/bin/sh");
        command
            .args(["-eu", "-c", &self.command])
            .stdin(std::process::Stdio::null())
            .stdout(std::io::stderr().as_fd().try_clone_to_owned()?)
            .env_clear()
            .env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            )
            .env("HOME", "/root")
            .env("LANG", "C");
        if let Some(root) = &self.carapace {
            command.env("CARAPACE", root);
        }
        success(command.status()?)
    }
}

fn mountpoint(root: &Path, name: &str) -> Result<(), Error> {
    let path = root.join(name);
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(Error::Invalid(format!(
            "runtime mountpoint /{name} must be a directory"
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(std::fs::create_dir(path)?)
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn private_mounts() -> Result<(), Error> {
    mount(
        None::<&str>,
        Path::new("/"),
        None::<&str>,
        MsFlags::MS_REC | MsFlags::MS_PRIVATE,
        None::<&str>,
    )?;
    Ok(())
}

fn die_with_parent(expected: Option<u32>) -> Result<(), Error> {
    set_pdeathsig(Signal::SIGKILL)?;
    // The inherited proc mount still uses the sender's PID view, even for the
    // new namespace's PID 1. Compare with the identity captured before spawn,
    // closing the race where the parent died before this helper started.
    let status = std::fs::read_to_string("/proc/self/status")?;
    let parent = status
        .lines()
        .find_map(|line| line.strip_prefix("PPid:")?.trim().parse::<u32>().ok());
    if expected.is_none() || parent != expected {
        return Err(Error::Invalid("step parent exited during startup".into()));
    }
    Ok(())
}

fn success(status: ExitStatus) -> Result<(), Error> {
    if status.success() {
        Ok(())
    } else {
        Err(Error::Invalid(format!("build step failed: {status}")))
    }
}

fn wait_child(mut child: std::process::Child, terminate: Signal) -> Result<ExitStatus, Error> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if let Err(error) = crate::cancel::check() {
            let pid = nix::unistd::Pid::from_raw(
                i32::try_from(child.id())
                    .map_err(|_| Error::Invalid("child PID out of range".into()))?,
            );
            let _ = nix::sys::signal::kill(pid, terminate);
            child.wait()?;
            return Err(error);
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
