// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CommandDevices, PrivateMountNamespace};
use crate::process::{CommandPrivileges, CommandSyscalls};
use rustix::mount::{MoveMountFlags, OpenTreeFlags, UnmountFlags};
use std::{
    fs::{self, File},
    io,
    marker::PhantomData,
    os::unix::process::CommandExt,
    process::{Command, Stdio},
    rc::Rc,
};

/// A pinned command root and trusted null input, prepared before switching roots.
/// The caller must supply its assigned mounted image in an isolated setup
/// process. Procfs and empty read-only sys are prepared here; the other runtime
/// views (including private devices) are not implemented yet. This is not admission authority:
/// PID/user/network isolation, containment and policy must be established by
/// the enclosing step. No raw root handle is returned to callers.
///
/// ```compile_fail
/// use conglobate::isolation::{CommandRoot, CommandDevices, PrivateMountNamespace};
/// use std::fs::File;
/// fn cannot_restore_with_selected_root(scope: PrivateMountNamespace, root: File, devices: CommandDevices) {
///     let root = CommandRoot::from_directory(root, &scope, devices).unwrap();
///     scope.restore();
///     drop(root);
/// }
/// ```
#[derive(Debug)]
pub struct CommandRoot<'ns> {
    scope: &'ns PrivateMountNamespace,
    root: File,
    devices: CommandDevices,
    input: File,
    _thread: PhantomData<Rc<()>>,
}

/// The current thread has irreversibly pivoted and detached its old root.
/// Failure after entering is exit-only; there is no restoration of trusted
/// supervisor authority. The enclosing process owns namespace teardown.
#[derive(Debug)]
pub struct EnteredRoot<'ns> {
    _scope: &'ns PrivateMountNamespace,
    input: File,
    _thread: PhantomData<Rc<()>>,
}

impl<'ns> CommandRoot<'ns> {
    /// Open the supplied directory after entering scope; inherited descriptors
    /// from a previous mount namespace cannot identify its cloned mount tree.
    pub fn from_directory(
        root: File,
        scope: &'ns PrivateMountNamespace,
        devices: CommandDevices,
    ) -> io::Result<Self> {
        if !root.metadata()?.is_dir() {
            return Err(io::Error::other("command root must be a directory"));
        }
        Ok(Self {
            scope,
            root,
            devices,
            input: File::open("/dev/null")?,
            _thread: PhantomData,
        })
    }

    /// Consume the selected root. After any error the setup process must exit:
    /// namespace/cwd/root transitions may already have happened. Call only from
    /// a single-threaded helper, never the build supervisor. No destructor is
    /// asserted to undo a root transition or finalize the filesystem.
    pub fn enter(self) -> io::Result<EnteredRoot<'ns>> {
        if fs::read_dir("/proc/self/task")?
            .collect::<Result<Vec<_>, _>>()?
            .len()
            != 1
        {
            return Err(io::Error::other("root setup requires one thread"));
        }
        super::runtime::RuntimeTargets::resolve(&self.root)?.install(self.devices)?;
        // The scope was entered before this directory was opened. Unsharing
        // here would leave the pinned descriptor referring to the old mount
        // namespace rather than its cloned tree.
        // Clone and attach through descriptors so cwd references the new mount,
        // not the covered mount underneath a pathname-based self bind.
        let tree = rustix::mount::open_tree(
            &self.root,
            "",
            OpenTreeFlags::AT_EMPTY_PATH
                | OpenTreeFlags::AT_RECURSIVE
                | OpenTreeFlags::OPEN_TREE_CLONE
                | OpenTreeFlags::OPEN_TREE_CLOEXEC,
        )?;
        rustix::mount::move_mount(
            &tree,
            "",
            &self.root,
            "",
            MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH | MoveMountFlags::MOVE_MOUNT_T_EMPTY_PATH,
        )?;
        rustix::process::fchdir(&tree)?;
        rustix::process::pivot_root(".", ".")
            .map_err(|error| io::Error::other(format!("pivot command root: {error}")))?;
        rustix::mount::unmount(".", UnmountFlags::DETACH)
            .map_err(|error| io::Error::other(format!("detach old root: {error}")))?;
        rustix::process::chdir("/")
            .map_err(|error| io::Error::other(format!("enter final root: {error}")))?;
        drop(tree);
        drop(self.root);
        Ok(EnteredRoot {
            _scope: self.scope,
            input: self.input,
            _thread: PhantomData,
        })
    }
}

impl EnteredRoot<'_> {
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
        let proc = File::open("/proc/self/fdinfo")?;
        if rustix::fs::fstatfs(&proc)?.f_type != 0x9fa0 {
            return Err(io::Error::other("descriptor audit requires real procfs"));
        }
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
                return Err(io::Error::other(format!(
                    "outside descriptor {fd} would survive command exec"
                )));
            }
        }
        Ok(())
    }
}
