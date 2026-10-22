// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::process::{CommandPrivileges, CommandSyscalls};
use nix::sched::{CloneFlags, unshare};
use rustix::mount::{MountPropagationFlags, UnmountFlags};
use std::{
    fs::{self, File},
    io,
    marker::PhantomData,
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    rc::Rc,
};

/// A pinned command root and trusted null input, prepared before switching roots.
/// The caller must supply its assigned mounted image with runtime mounts already
/// prepared, in an isolated setup process. This is not admission authority:
/// PID/user/network isolation, containment and policy must be established by
/// the enclosing step. No raw root handle is returned to callers.
#[derive(Debug)]
pub struct CommandRoot {
    root: File,
    input: File,
    _thread: PhantomData<Rc<()>>,
}

/// The current thread has irreversibly pivoted and detached its old root.
/// Failure after entering is exit-only; there is no restoration of trusted
/// supervisor authority. The enclosing process owns namespace teardown.
#[derive(Debug)]
pub struct EnteredRoot {
    input: File,
    _thread: PhantomData<Rc<()>>,
}

impl CommandRoot {
    pub fn from_directory(root: File) -> io::Result<Self> {
        if !root.metadata()?.is_dir() {
            return Err(io::Error::other("command root must be a directory"));
        }
        Ok(Self {
            root,
            input: File::open("/dev/null")?,
            _thread: PhantomData,
        })
    }

    /// Consume the selected root. After any error the setup process must exit:
    /// namespace/cwd/root transitions may already have happened. Call only from
    /// a single-threaded helper, never the build supervisor. No destructor is
    /// asserted to undo a root transition or finalize the filesystem.
    pub fn enter(self) -> io::Result<EnteredRoot> {
        if fs::read_dir("/proc/self/task")?
            .collect::<Result<Vec<_>, _>>()?
            .len()
            != 1
        {
            return Err(io::Error::other("root setup requires one thread"));
        }
        // NEWNS also unshares fs_struct, so cwd/root changes do not affect the
        // parent. Make propagation private before detaching the old root.
        unshare(CloneFlags::CLONE_NEWNS)?;
        rustix::mount::mount_change(
            "/",
            MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
        )?;
        rustix::process::fchdir(&self.root)?;
        rustix::process::pivot_root(".", ".")?;
        rustix::mount::unmount(".", UnmountFlags::DETACH)?;
        rustix::process::chdir("/")?;
        drop(self.root);
        Ok(EnteredRoot {
            input: self.input,
            _thread: PhantomData,
        })
    }
}

impl EnteredRoot {
    /// Finalize native restrictions in already-loaded trusted code, then replace
    /// this process with the image shell. A returned error is always fatal to
    /// setup. This initial entrypoint has no CARAPACE environment support yet.
    pub fn execute(
        self,
        script: &str,
        privileges: CommandPrivileges,
        syscalls: CommandSyscalls,
    ) -> io::Result<std::convert::Infallible> {
        self.check_descriptors()?;
        privileges.apply().map_err(io::Error::other)?;
        syscalls.apply()?;
        let error = Command::new("/bin/sh")
            .args(["-eu", "-c", script])
            .current_dir("/")
            .env_clear()
            .env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            )
            .env("HOME", "/root")
            .env("LANG", "C.UTF-8")
            .stdin(Stdio::from(self.input))
            .exec();
        Err(error)
    }

    fn check_descriptors(&self) -> io::Result<()> {
        // Probe through procfs rather than manufacturing borrowed raw FDs. The
        // only permitted inherited descriptors are diagnostic stdout/stderr and
        // stdin, which execute replaces. All other descriptors must close on exec.
        for entry in fs::read_dir("/proc/self/fdinfo")? {
            let entry = entry?;
            let name = entry.file_name();
            let fd: u32 = name
                .to_str()
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| io::Error::other("invalid proc descriptor entry"))?;
            if fd <= 2 {
                continue;
            }
            let info = fs::read_to_string(entry.path())?;
            let flags = info
                .lines()
                .find_map(|line| line.strip_prefix("flags:\t"))
                .and_then(|v| u32::from_str_radix(v, 8).ok())
                .ok_or_else(|| io::Error::other("missing descriptor flags"))?;
            if flags & rustix::fs::OFlags::CLOEXEC.bits() == 0 {
                return Err(io::Error::other(
                    "outside descriptor would survive command exec",
                ));
            }
        }
        Ok(())
    }
}
