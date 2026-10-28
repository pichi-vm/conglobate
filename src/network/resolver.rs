// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use rustix::fs::{Mode, OFlags};
use std::{
    fmt,
    fs::File,
    io::{self, Read},
    os::fd::AsRawFd,
    path::Path,
};

/// The environment's exact resolver bytes, captured once for a build. There is
/// no source pathname or writable handle to reopen between steps. This input is
/// deliberately excluded from build annotations, and Debug never renders its contents.
/// Resolver syntax and reachable DNS service are supplied by the environment;
/// conglobate does not rewrite stubs or infer servers/search domains.
pub(crate) struct ResolverInput {
    bytes: Box<[u8]>,
}
impl fmt::Debug for ResolverInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResolverInput")
            .finish_non_exhaustive()
    }
}
impl ResolverInput {
    pub(crate) fn capture(path: &Path) -> io::Result<Self> {
        // Pin without activating devices or blocking on a FIFO. Reopening the
        // validated inode via proc prevents source-path replacement between the
        // type check and read from redirecting capture to a different object.
        let pinned = File::from(rustix::fs::open(
            path,
            OFlags::PATH | OFlags::CLOEXEC,
            Mode::empty(),
        )?);
        let metadata = pinned.metadata()?;
        if !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "resolver input must be a regular file",
            ));
        }
        let capacity = metadata.len().checked_add(1).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "resolver input size overflow")
        })?;
        let length = usize::try_from(capacity).map_err(io::Error::other)?;
        let file = File::from(rustix::fs::open(
            format!("/proc/self/fd/{}", pinned.as_raw_fd()),
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )?);
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(io::Error::other)?;
        file.take(capacity).read_to_end(&mut bytes)?;
        if bytes.len() as u64 != metadata.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "resolver input size changed during capture",
            ));
        }
        Ok(Self {
            bytes: bytes.into_boxed_slice(),
        })
    }
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::{fs::symlink, net::UnixListener},
    };

    #[test]
    fn captures_symlink_once_and_keeps_exact_bytes_after_source_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("resolved");
        let path = directory.path().join("resolv.conf");
        let bytes = b"nameserver 192.0.2.53\nsearch internal.test\n# opaque byte: \xff\n";
        fs::write(&source, bytes).unwrap();
        symlink(&source, &path).unwrap();
        let input = ResolverInput::capture(&path).unwrap();
        fs::remove_file(&source).unwrap();
        fs::write(&source, b"changed").unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!(input.bytes(), bytes);
        let diagnostic = format!("{input:?}");
        assert!(!diagnostic.contains("192.0.2.53"));
        assert!(!diagnostic.contains("internal.test"));
        assert!(!diagnostic.contains("resolved"));
    }

    #[test]
    fn rejects_nonregular_sources_without_reading_or_waiting_for_a_writer() {
        let directory = tempfile::tempdir().unwrap();
        let fifo = directory.path().join("fifo");
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &fifo,
            rustix::fs::FileType::Fifo,
            Mode::RUSR | Mode::WUSR,
            0,
        )
        .unwrap();
        let socket = directory.path().join("socket");
        let _listener = UnixListener::bind(&socket).unwrap();
        for path in [
            directory.path(),
            fifo.as_path(),
            socket.as_path(),
            Path::new("/dev/null"),
        ] {
            assert_eq!(
                ResolverInput::capture(path).unwrap_err().kind(),
                io::ErrorKind::InvalidInput
            );
        }
        let cycle = directory.path().join("cycle");
        symlink("cycle", &cycle).unwrap();
        assert!(ResolverInput::capture(&cycle).is_err());
        assert!(ResolverInput::capture(&directory.path().join("missing")).is_err());
    }

    #[test]
    fn does_not_invent_configuration_for_an_empty_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        assert!(
            ResolverInput::capture(file.path())
                .unwrap()
                .bytes()
                .is_empty()
        );
    }
}
