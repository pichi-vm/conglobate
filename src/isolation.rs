// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Trusted helper namespace setup; this alone is not a command sandbox.
mod devices;
mod gate;
mod init;
mod namespaces;
mod setup;
pub use gate::{ExecutionEndpoint, ExecutionGate, GateState, Ready, Waiting};
mod user;
pub use namespaces::CommandNamespaces;
pub use setup::{CommandSetup, GatedCommandSetup, GatedRootSetup};
pub use user::{
    IdRange, Mapped, NamespaceViewFailure, PinnedNamespace, UserNamespace, UserNamespaceFailure,
    UserNamespaceReleaseFailure, UserNamespaceState,
};
mod root;
mod runtime;
pub use devices::CommandDevices;
use nix::sched::{CloneFlags, setns, unshare};
pub use root::{CommandRoot, EnteredRoot, RestrictedCommand};
use rustix::mount::MountPropagationFlags;
use std::{
    fs::{self, File},
    io,
    marker::PhantomData,
    rc::Rc,
};

#[derive(Debug)]
struct NamespaceLease {
    original: Option<File>,
    _thread: PhantomData<Rc<()>>,
}
/// Private mount namespace for a dedicated single-threaded setup helper.
/// Call before starting threads. The thread-count check diagnoses misuse; it
/// cannot prevent unrelated code from racing to create another thread.
/// No public raw namespace handle or arbitrary unshare flags are exposed.
///
/// ```compile_fail
/// use conglobate::{filesystem::{Mount, Detached}, isolation::PrivateMountNamespace};
/// use std::fs::File;
/// fn cannot_restore_with_live_mount(scope: PrivateMountNamespace, root: File) {
///     let mount = Mount::<Detached>::clone_from(&root, &scope).unwrap();
///     scope.restore();
///     drop(mount);
/// }
/// ```
#[derive(Debug)]
pub struct PrivateMountNamespace {
    lease: NamespaceLease,
    current: File,
}
#[derive(Debug, thiserror::Error)]
pub enum NamespaceFailure {
    #[error("namespace setup before transition: {0}")]
    Before(io::Error),
    #[error("namespace operation after transition: {source}")]
    Entered {
        source: io::Error,
        recovery: NamespaceRecovery,
    },
}
#[derive(Debug)]
pub struct NamespaceRecovery {
    lease: NamespaceLease,
}
impl NamespaceFailure {
    pub fn restore(self) -> Result<(), Self> {
        match self {
            Self::Entered { recovery, .. } => recovery.lease.restore(),
            error => Err(error),
        }
    }
}
impl PrivateMountNamespace {
    pub fn enter() -> Result<Self, NamespaceFailure> {
        let tasks = fs::read_dir("/proc/self/task")
            .map_err(NamespaceFailure::Before)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(NamespaceFailure::Before)?;
        if tasks.len() != 1 {
            return Err(NamespaceFailure::Before(io::Error::other(
                "namespace setup requires a single-threaded helper",
            )));
        }
        let original = File::open("/proc/thread-self/ns/mnt").map_err(NamespaceFailure::Before)?;
        // NEWNS also unshares fs_struct, not the descriptor table. Never expose
        // caller-selected flags or use CLONE_FILES through this safe interface.
        unshare(CloneFlags::CLONE_NEWNS).map_err(|e| NamespaceFailure::Before(e.into()))?;
        let lease = NamespaceLease {
            original: Some(original),
            _thread: PhantomData,
        };
        if let Err(source) = rustix::mount::mount_change(
            "/",
            MountPropagationFlags::PRIVATE | MountPropagationFlags::REC,
        ) {
            return Err(NamespaceFailure::Entered {
                source: source.into(),
                recovery: NamespaceRecovery { lease },
            });
        }
        let current = match File::open("/proc/thread-self/ns/mnt") {
            Ok(current) => current,
            Err(source) => {
                return Err(NamespaceFailure::Entered {
                    source,
                    recovery: NamespaceRecovery { lease },
                });
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
    pub fn restore(self) -> Result<(), NamespaceFailure> {
        self.lease.restore()
    }
}
impl NamespaceLease {
    fn restore(mut self) -> Result<(), NamespaceFailure> {
        if let Err(source) = setns(
            self.original.as_ref().expect("live namespace lease"),
            CloneFlags::CLONE_NEWNS,
        ) {
            return Err(NamespaceFailure::Entered {
                source: source.into(),
                recovery: NamespaceRecovery { lease: self },
            });
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
