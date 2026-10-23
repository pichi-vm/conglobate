// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::process::{Process, Running};
use nix::sched::{CloneFlags, setns, unshare};
use rustix::{
    mount::MountPropagationFlags,
    process::{Gid, Uid},
};
use std::{
    fs::{self, File},
    io,
    marker::PhantomData,
    process::Command,
    rc::Rc,
};

/// Namespace preparation in a fresh single-threaded trusted helper. The helper
/// irreversibly joins the supplied command user namespace, maps itself to root,
/// and creates private mount/network/IPC/UTS namespaces plus a PID namespace for
/// its next child. It retains no namespace FD and has no restoration authority.
///
/// This is not sandbox admission: the child must be trusted root-entry code
/// which pivots, installs runtime mounts and drops capabilities/syscalls before
/// executing image commands. The outer step must already own containment/policy.
/// Any error after entry is exit-only. Never call from the build supervisor.
/// Do not create other children or threads between entry and spawning init.
///
/// ```compile_fail
/// use conglobate::isolation::CommandNamespaces;
/// fn cannot_spawn_from_another_thread(scope: CommandNamespaces) {
///     std::thread::spawn(move || scope.spawn(std::process::Command::new("/unused")));
/// }
/// ```
#[derive(Debug)]
pub struct CommandNamespaces {
    _thread: PhantomData<Rc<()>>,
}
impl CommandNamespaces {
    pub fn enter(user_namespace: File) -> io::Result<Self> {
        if fs::read_dir("/proc/self/task")?
            .collect::<io::Result<Vec<_>>>()?
            .len()
            != 1
        {
            return Err(io::Error::other(
                "command namespace entry requires one thread",
            ));
        }
        if rustix::fs::fstatfs(&user_namespace)?.f_type as u64 != 0x6e73_6673 {
            return Err(io::Error::other("command user namespace requires nsfs"));
        }
        // Drop inherited supplementary membership while initial-namespace
        // authority is still available; the selected namespace may deny setgroups.
        rustix::thread::set_thread_groups(&[])?;
        setns(&user_namespace, CloneFlags::CLONE_NEWUSER)?;
        drop(user_namespace);
        rustix::thread::set_thread_gid(Gid::ROOT)?;
        rustix::thread::set_thread_uid(Uid::ROOT)?;
        unshare(
            CloneFlags::CLONE_NEWNS
                | CloneFlags::CLONE_NEWPID
                | CloneFlags::CLONE_NEWNET
                | CloneFlags::CLONE_NEWIPC
                | CloneFlags::CLONE_NEWUTS,
        )?;
        rustix::mount::mount_change(
            "/",
            MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
        )?;
        Ok(Self {
            _thread: PhantomData,
        })
    }

    /// Spawn exactly the next trusted setup child, which becomes PID 1. The
    /// returned process owns termination/reaping; this helper must then wait and
    /// exit, never attempt to regain supervisor namespaces or continue a build.
    pub fn spawn(self, mut setup: Command) -> io::Result<Process<Running>> {
        setup.spawn().map(Process::from)
    }
}
