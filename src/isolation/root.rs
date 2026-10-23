// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CommandDevices, PrivateMountNamespace};
use crate::process::{CommandPrivileges, CommandSyscalls};
use rustix::mount::{MoveMountFlags, OpenTreeFlags, UnmountFlags};
use std::{
    fs::{self, File},
    io,
    marker::PhantomData,
    process::Command,
    rc::Rc,
};

/// A pinned command root and trusted null input, prepared before switching roots.
/// The caller must supply its assigned mounted image in an isolated setup
/// process. Procfs, empty read-only sys and supplied private devices are prepared
/// here, including private devpts/shm. This is not admission authority:
/// PID/user/network isolation, containment and policy must be established by
/// the enclosing step. No raw root handle is returned to callers.
///
/// ```compile_fail
/// use conglobate::isolation::{CommandRoot, CommandDevices, PrivateMountNamespace};
/// fn cannot_restore_with_selected_root(scope: PrivateMountNamespace, devices: CommandDevices) {
///     let root = CommandRoot::from_current_directory(scope, devices).unwrap();
///     scope.restore();
///     drop(root);
/// }
/// ```
#[derive(Debug)]
pub struct CommandRoot {
    scope: PrivateMountNamespace,
    root: File,
    devices: CommandDevices,
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

/// Trusted command prepared in its final root with capabilities and syscalls
/// restricted. No image code has run. A start handshake may occur before
/// execute, but every extra descriptor must be closed before that call.
/// ```compile_fail
/// use conglobate::isolation::RestrictedCommand;
/// fn cannot_run_twice(command: RestrictedCommand) {
///     command.execute();
///     command.execute();
/// }
/// ```
#[derive(Debug)]
pub struct RestrictedCommand {
    command: Command,
    _thread: PhantomData<Rc<()>>,
}

impl CommandRoot {
    /// Capture the cwd assigned by the mounted-root launcher after entering
    /// scope. The kernel retargets cwd across namespace cloning, unlike a file
    /// descriptor opened in a previous mount namespace.
    pub fn from_current_directory(
        scope: PrivateMountNamespace,
        devices: CommandDevices,
    ) -> io::Result<Self> {
        let root = File::open(".")?;
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
    pub fn enter(self) -> io::Result<EnteredRoot> {
        if fs::read_dir("/proc/self/task")?
            .collect::<Result<Vec<_>, _>>()?
            .len()
            != 1
        {
            return Err(io::Error::other("root setup requires one thread"));
        }
        // The setup helper is a fresh child, not a process-group leader. A
        // separate session severs any inherited controlling terminal before
        // exposing the private tty device. Failure is fatal to command setup.
        rustix::process::setsid()?;
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
        self.scope.commit();
        Ok(EnteredRoot {
            input: self.input,
            _thread: PhantomData,
        })
    }
}

impl EnteredRoot {
    /// Finalize restrictions and remain as the command PID namespace's init,
    /// reaping descendants while the image shell runs. Exits with the shell's
    /// outcome; PID namespace destruction and outer containment stop survivors.
    /// A returned error is fatal: this helper must exit. No CARAPACE support yet.
    pub fn execute(
        self,
        script: &str,
        privileges: CommandPrivileges,
        syscalls: CommandSyscalls,
    ) -> io::Result<std::convert::Infallible> {
        self.restrict(script, privileges, syscalls)?.execute()
    }

    /// Finish trusted isolation before the execution clock starts. This does
    /// not grant command admission; the enclosing helper owns the start gate.
    /// Errors remain exit-only because privilege changes cannot be rolled back.
    pub fn restrict(
        self,
        script: &str,
        privileges: CommandPrivileges,
        syscalls: CommandSyscalls,
    ) -> io::Result<RestrictedCommand> {
        rustix::stdio::dup2_stdin(&self.input)?;
        drop(self.input);
        privileges.apply().map_err(io::Error::other)?;
        // Resident init must not expose its executable/memory through procfs
        // to commands with the same UID. Image exec resets the child's setting.
        nix::sys::prctl::set_dumpable(false)?;
        syscalls.apply()?;
        let mut command = Command::new("/bin/sh");
        command
            .args(["-eu", "-c", script])
            .current_dir("/")
            .env_clear()
            .env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            )
            .env("HOME", "/root")
            .env("LANG", "C.UTF-8");
        Ok(RestrictedCommand {
            command,
            _thread: PhantomData,
        })
    }
}

impl RestrictedCommand {
    /// Report completed isolation, wait for explicit start, close the gate and
    /// audit descriptors before image execution. Error is exit-only.
    pub fn start(
        self,
        gate: super::ExecutionEndpoint,
        deadline: std::time::Instant,
    ) -> io::Result<std::convert::Infallible> {
        gate.ready_and_wait(deadline)?;
        self.execute()
    }

    /// The enclosing helper must have received start authorization and closed
    /// its startup handles. Reaudit immediately before spawning image code.
    pub fn execute(self) -> io::Result<std::convert::Infallible> {
        self.check_descriptors()?;
        super::init::NamespaceInit::spawn(self.command)?.execute()
    }

    fn check_descriptors(&self) -> io::Result<()> {
        let proc = File::open("/proc/self/fdinfo")?;
        if rustix::fs::fstatfs(&proc)?.f_type != 0x9fa0 {
            return Err(io::Error::other("descriptor audit requires real procfs"));
        }
        drop(proc);
        // Init remains resident instead of execing: CLOEXEC alone would leave
        // outside handles visible through its /proc/1/fd. Reject every extra
        // descriptor. The owned null input was installed and closed by restrict.
        // Collect names first so the audit's own directory descriptors close.
        let names = fs::read_dir("/proc/self/fdinfo")?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<io::Result<Vec<_>>>()?;
        for name in names {
            let fd: i32 = name
                .to_str()
                .and_then(|n| n.parse().ok())
                .ok_or_else(|| io::Error::other("invalid proc descriptor entry"))?;
            if fd <= 2 {
                continue;
            }
            match fs::metadata(std::path::Path::new("/proc/self/fdinfo").join(name)) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
                Ok(_) => {
                    return Err(io::Error::other(format!(
                        "outside descriptor {fd} would remain in command init"
                    )));
                }
            }
        }
        Ok(())
    }
}
