// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{Process, Running};
use std::{
    io,
    thread::{self, JoinHandle},
};

/// Bounded streaming diagnostic transport in the trusted outer receiver,
/// outside the command PID namespace. Only pipe write ends reach its child;
/// the supervisor's stdout/stderr descriptors never reach command code.
/// Join only after PID 1 has exited and the kernel has stopped its descendants.
/// The outer supervisor bounds this helper's lifetime and cancels its domain.
#[derive(Debug)]
pub(crate) struct DiagnosticRelay {
    stdout: JoinHandle<io::Result<u64>>,
    stderr: JoinHandle<io::Result<u64>>,
}
impl DiagnosticRelay {
    pub(crate) fn finish(self) -> io::Result<()> {
        let stdout = self
            .stdout
            .join()
            .map_err(|_| io::Error::other("stdout relay panicked"))?;
        let stderr = self
            .stderr
            .join()
            .map_err(|_| io::Error::other("stderr relay panicked"))?;
        stdout?;
        stderr?;
        Ok(())
    }
}
impl Process<Running> {
    /// Exit-only helper path: call immediately after spawning the trusted
    /// namespace-entry child with piped stdout/stderr, before another spawn or any
    /// namespace transition. A relay error rejects helper completion.
    pub(crate) fn relay_diagnostics(&mut self) -> io::Result<DiagnosticRelay> {
        let child = self.resources.child.as_mut().expect("live child");
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("missing stdout pipe"))?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("missing stderr pipe"))?;
        // Stdout/Stderr lock each write; never hold the global diagnostic lock
        // while waiting for child output, including on startup error paths.
        let stdout = thread::Builder::new()
            .name("command-stdout".into())
            .spawn(move || io::copy(&mut stdout, &mut io::stdout()))?;
        let stderr = thread::Builder::new()
            .name("command-stderr".into())
            .spawn(move || io::copy(&mut stderr, &mut io::stderr()))?;
        Ok(DiagnosticRelay { stdout, stderr })
    }
}
