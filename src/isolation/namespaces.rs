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
    time::Instant,
};

/// Namespace preparation in a fresh single-threaded trusted helper. The helper
/// irreversibly joins the supplied command user namespace, maps itself to root,
/// and creates private mount/IPC/UTS namespaces plus a PID namespace for
/// its next child. Offline entry creates a fresh network namespace; configured
/// entry joins the selected network before entering the user namespace. It
/// retains no namespace FD and has no restoration authority.
///
/// This is not sandbox admission: the child must be trusted root-entry code
/// which pivots, installs runtime mounts and drops capabilities/syscalls before
/// executing image commands. The outer step must already own containment and mapped namespace resources.
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
    pub(super) fn enter_with_network(
        user_namespace: File,
        network: Option<File>,
        deadline: Instant,
    ) -> io::Result<Self> {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "namespace setup deadline expired",
            ));
        }
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
        let network_flags = match network {
            Some(network) => {
                use std::os::unix::fs::MetadataExt;
                let selected = network.metadata()?;
                let current = fs::metadata("/proc/self/ns/net")?;
                if (selected.dev(), selected.ino()) == (current.dev(), current.ino()) {
                    return Err(io::Error::other(
                        "command network must differ from supervisor network",
                    ));
                }
                // This namespace is owned by the supervisor's user namespace.
                // Join it before surrendering that authority, then close the FD.
                setns(&network, CloneFlags::CLONE_NEWNET)?;
                drop(network);
                CloneFlags::empty()
            }
            None => CloneFlags::CLONE_NEWNET,
        };
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
                | network_flags
                | CloneFlags::CLONE_NEWIPC
                | CloneFlags::CLONE_NEWUTS,
        )?;
        if network_flags.contains(CloneFlags::CLONE_NEWNET) {
            crate::network::Loopback::activate(deadline)?;
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expired_setup_fails_before_namespace_entry() {
        let user = File::open("/proc/self/ns/user").unwrap();
        let error = CommandNamespaces::enter_with_network(user, None, Instant::now()).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    }
}
