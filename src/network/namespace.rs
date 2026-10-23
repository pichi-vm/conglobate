// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use std::{
    fs::{self, File},
    io,
    os::unix::fs::MetadataExt,
};

/// Pinned environment-owned network namespace, captured before command user
/// namespaces exist. Descriptor release does not tear down external networking.
#[derive(Debug)]
pub struct NetworkNamespace(File);
impl NetworkNamespace {
    /// Pin an environment-owned namespace. No network configuration is changed.
    pub fn open(path: &std::path::Path) -> io::Result<Self> {
        let file: File = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY | rustix::fs::OFlags::CLOEXEC | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )?
        .into();
        if rustix::fs::fstatfs(&file)?.f_type as u64 != 0x6e73_6673 {
            return Err(io::Error::other("network namespace input requires nsfs"));
        }
        let selected = file.metadata()?;
        // Bind-mounted namespace FD links identify the mount path, not its
        // type. The trusted receiver validates type with setns(CLONE_NEWNET)
        // before command admission; capture never changes the caller's netns.
        let ours = fs::metadata("/proc/self/ns/net")?;
        if (ours.dev(), ours.ino()) == (selected.dev(), selected.ino()) {
            return Err(io::Error::other(
                "command network must differ from supervisor network",
            ));
        }
        Ok(Self(file))
    }

    pub(crate) fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }

    pub(crate) fn descriptor(&self) -> &File {
        &self.0
    }

    /// Read-only identity for lifecycle diagnostics, not an execution handle.
    pub fn metadata(&self) -> io::Result<fs::Metadata> {
        self.0.metadata()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn external_namespace_rejects_file_and_supervisor() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(
            NetworkNamespace::open(file.path())
                .unwrap_err()
                .to_string()
                .contains("requires nsfs")
        );
        assert!(
            NetworkNamespace::open(Path::new("/proc/self/ns/net"))
                .unwrap_err()
                .to_string()
                .contains("differ")
        );
    }
}
