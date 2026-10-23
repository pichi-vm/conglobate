// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    execution::HelperProgram,
    process::{Identified, LaunchFailure, Process, Supervision, WaitFailure},
    step::SetupCutoff,
};
use nix::sched::{CloneFlags, unshare};
use std::{
    fs::{self, File},
    io,
    os::unix::fs::MetadataExt,
    process::ExitStatus,
};

/// A fresh network namespace owned by the supervisor's user namespace. Its
/// trusted creator is reaped before this value becomes available. Closing the
/// final descriptor releases it once no tasks or links retain its lifetime.
/// Commands must enter this namespace before dropping initial-user-namespace
/// authority; they never receive its descriptor as an execution resource.
#[derive(Debug)]
pub struct NetworkNamespace(File);

#[derive(Debug, thiserror::Error)]
pub enum NetworkNamespaceFailure {
    #[error("network namespace launch: {0}")]
    Launch(#[from] LaunchFailure),
    #[error("network namespace capture: {source}")]
    Capture {
        source: io::Error,
        process: Process<Identified>,
    },
    #[error("network namespace creator reap: {0}")]
    Reap(#[from] WaitFailure<Identified>),
    #[error("network namespace creator failed: {0}")]
    Exited(ExitStatus),
    #[error("network namespace input: {0}")]
    Input(#[from] io::Error),
}
impl NetworkNamespace {
    pub fn create(
        program: &HelperProgram,
        cutoff: SetupCutoff,
    ) -> Result<Self, NetworkNamespaceFailure> {
        let deadline = cutoff.deadline()?;
        let (process, channel) =
            Process::launch_supervised(program.network_namespace_creator(cutoff)?, deadline)?;
        let captured = Self::capture(&process);
        // The ready creator stays alive until its namespace has an independent
        // descriptor owner. Channel closure releases it without readiness sleeps.
        drop(channel);
        let namespace = match captured {
            Ok(namespace) => namespace,
            Err(source) => return Err(NetworkNamespaceFailure::Capture { source, process }),
        };
        let status = process.wait_until(deadline)?.status();
        if !status.success() {
            return Err(NetworkNamespaceFailure::Exited(status));
        }
        Ok(namespace)
    }
    fn capture(process: &Process<Identified>) -> io::Result<Self> {
        let task = process.proc_directory()?;
        let namespace: File = rustix::fs::openat(
            &task,
            "ns/net",
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?
        .into();
        let ours = fs::metadata("/proc/self/ns/net")?;
        let selected = namespace.metadata()?;
        if (ours.dev(), ours.ino()) == (selected.dev(), selected.ino()) {
            return Err(io::Error::other(
                "creator did not create a network namespace",
            ));
        }
        let owner = rustix::fs::statat(&task, "ns/user", rustix::fs::AtFlags::empty())?;
        let expected = fs::metadata("/proc/self/ns/user")?;
        if (owner.st_dev, owner.st_ino) != (expected.dev(), expected.ino()) {
            return Err(io::Error::other("network creator changed user namespace"));
        }
        Ok(Self(namespace))
    }
    pub(crate) fn descriptor(&self) -> &File {
        &self.0
    }

    /// Read-only identity for lifecycle diagnostics, not an execution handle.
    pub fn metadata(&self) -> io::Result<fs::Metadata> {
        self.0.metadata()
    }

    /// Trusted fresh helper only. No image code or restoration authority.
    pub(crate) fn serve(deadline: std::time::Instant) -> io::Result<()> {
        if fs::read_dir("/proc/self/task")?.count() != 1 {
            return Err(io::Error::other(
                "network namespace creation requires one thread",
            ));
        }
        let channel = Supervision::from_stdin()?;
        unshare(CloneFlags::CLONE_NEWNET)?;
        channel
            .exchange_until(deadline)?
            .wait_for_loss_until(deadline)
    }
}
