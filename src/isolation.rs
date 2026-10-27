// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Trusted helper namespace setup; this alone is not a command sandbox.
mod devices;
mod labels;
pub(crate) use labels::AssignedRoots;
pub(crate) mod receiver;
pub(crate) use receiver::CommandReceiver;
mod gate;
mod init;
mod namespaces;
mod setup;
pub(crate) use gate::{
    CompletionEndpoint, ExecutionEndpoint, ExecutionGate, Ready as GateReady, Started,
};
mod user;
pub(crate) use namespaces::CommandNamespaces;
pub(crate) use setup::{CommandSetup, GatedCommandSetup, GatedRootSetup};
pub(crate) use user::{IdMappings, IdRange, Mapped, UserNamespace};
mod root;
mod runtime;
mod shell;
pub(crate) use devices::CommandDevices;
use nix::sched::{CloneFlags, setns, unshare};
pub(crate) use root::{CommandRoot, RestrictedCommand};
use rustix::mount::MountPropagationFlags;
use std::{fs::File, io, marker::PhantomData, rc::Rc};

#[derive(Debug)]
struct NamespaceLease {
    original: Option<File>,
    _thread: PhantomData<Rc<()>>,
}
/// Private mount namespace for a dedicated single-threaded setup helper.
/// Call before starting threads. The thread-count check diagnoses misuse; it
/// cannot prevent unrelated code from racing to create another thread.
/// No public raw namespace handle or arbitrary unshare flags are exposed.
/// Drop attempts to restore the original namespace and only diagnoses failure.
/// Use explicit restoration to handle errors. Irreversible command-root entry
/// commits the guard, closing its restoration handle before command admission.
///
#[derive(Debug)]
pub(crate) struct PrivateMountNamespace {
    lease: NamespaceLease,
    current: File,
}
impl PrivateMountNamespace {
    pub(crate) fn enter() -> io::Result<Self> {
        let tasks = crate::process::ThreadSnapshot::capture()?;
        if !tasks.is_single() {
            return Err(io::Error::other(
                "namespace setup requires a single-threaded helper",
            ));
        }
        let original = File::open("/proc/thread-self/ns/mnt")?;
        // NEWNS also unshares fs_struct, not the descriptor table. Never expose
        // caller-selected flags or use CLONE_FILES through this safe interface.
        unshare(CloneFlags::CLONE_NEWNS)?;
        let lease = NamespaceLease {
            original: Some(original),
            _thread: PhantomData,
        };
        if let Err(source) = rustix::mount::mount_change(
            "/",
            MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
        ) {
            return Err(source.into());
        }
        let current = match File::open("/proc/thread-self/ns/mnt") {
            Ok(current) => current,
            Err(source) => {
                return Err(source);
            }
        };
        Ok(Self { lease, current })
    }
    pub(crate) fn verify_current(&self) -> io::Result<()> {
        use std::os::unix::fs::MetadataExt;
        let expected = self.current.metadata()?;
        let current = File::open("/proc/thread-self/ns/mnt")?.metadata()?;
        if (expected.dev(), expected.ino()) != (current.dev(), current.ino()) {
            return Err(io::Error::other(
                "private mount namespace scope is not current",
            ));
        }
        Ok(())
    }
    /// Commit the helper to its current namespace after irreversible root entry.
    /// Closing the restoration handle prevents the resident command init from
    /// retaining outside-namespace authority. There is no restoration on Drop.
    fn commit(mut self) {
        self.lease.original.take();
    }

    /// Restore the original namespace. Attached resources must be cleaned up
    /// before restoration. The helper may not admit commands on this basis alone.
    pub(crate) fn restore(self) -> io::Result<()> {
        self.lease.restore()
    }
}
impl NamespaceLease {
    fn restore(mut self) -> io::Result<()> {
        if let Err(source) = setns(
            self.original.as_ref().expect("live namespace lease"),
            CloneFlags::CLONE_NEWNS,
        ) {
            return Err(source.into());
        }
        self.original.take();
        Ok(())
    }
}
impl Drop for NamespaceLease {
    fn drop(&mut self) {
        if let Some(original) = self.original.take()
            && let Err(error) = setns(&original, CloneFlags::CLONE_NEWNS)
        {
            eprintln!("namespace restoration failed; helper must exit: {error}");
        }
    }
}

pub(crate) use runtime::RuntimeTargets;
