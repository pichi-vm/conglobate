// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::{CopyMode, CopySources, SelectionError};
use crate::{
    context::{CapturedContext, ContextEntry, ContextError},
    recipe::Copy,
};
use rustix::fs::{self, AtFlags, Mode, OFlags, ResolveFlags};
use std::{
    collections::VecDeque,
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Read, Write},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum CopyError {
    #[error(transparent)]
    Selection(#[from] SelectionError),
    #[error(transparent)]
    Context(#[from] ContextError),
    #[error("COPY filesystem: {0}")]
    Io(#[from] io::Error),
    #[error("COPY: {0}")]
    Invalid(&'static str),
}
impl From<rustix::io::Errno> for CopyError {
    fn from(error: rustix::io::Errno) -> Self {
        Self::Io(error.into())
    }
}
#[derive(Debug)]
pub(crate) struct CopyDestination<'a> {
    root: &'a File,
    output: Option<&'a File>,
}
#[derive(Debug)]
struct MetadataPolicy<'a> {
    uid: u32,
    gid: u32,
    mode: Option<&'a CopyMode>,
}
impl MetadataPolicy<'_> {
    fn file(&self, file: &File, mode: u32, directory: bool) -> Result<(), CopyError> {
        fs::fchown(
            file,
            Some(rustix::fs::Uid::from_raw(self.uid)),
            Some(rustix::fs::Gid::from_raw(self.gid)),
        )?;
        fs::fchmod(
            file,
            Mode::from_raw_mode(self.mode.map_or(mode, |m| m.apply(mode, directory))),
        )?;
        Ok(())
    }
    fn link(&self, parent: &File, name: &OsStr) -> Result<(), CopyError> {
        fs::chownat(
            parent,
            name,
            Some(rustix::fs::Uid::from_raw(self.uid)),
            Some(rustix::fs::Gid::from_raw(self.gid)),
            AtFlags::SYMLINK_NOFOLLOW,
        )?;
        Ok(())
    }
}
impl<'a> CopyDestination<'a> {
    pub(crate) fn new(root: &'a File) -> Self {
        Self { root, output: None }
    }
    pub(crate) fn bootstrap(root: &'a File, output: &'a File) -> Self {
        Self {
            root,
            output: Some(output),
        }
    }
    fn location<'p>(&self, path: &'p Path) -> (&File, &'p Path) {
        if let Some(output) = self.output
            && let Ok(relative) = path.strip_prefix("mnt")
        {
            return (output, relative);
        }
        (self.root, path)
    }
    fn open(&self, path: &Path, flags: OFlags) -> io::Result<File> {
        let (root, path) = self.location(path);
        let path = if path.as_os_str().is_empty() {
            Path::new(".")
        } else {
            path
        };
        Ok(fs::openat2(
            root,
            path,
            flags | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )?
        .into())
    }
    fn observe(&self, path: &Path) -> io::Result<Option<std::fs::Metadata>> {
        match self.open(path, OFlags::PATH | OFlags::NOFOLLOW) {
            Ok(file) => Ok(Some(file.metadata()?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }
    /// Resolve image symlinks with image-root semantics, never host-root semantics.
    /// O_PATH observations cannot activate image-controlled devices or FIFOs.
    fn resolve(&self, path: &Path, follow_final: bool) -> Result<PathBuf, CopyError> {
        let mut pending: VecDeque<OsString> = path.iter().map(OsString::from).collect();
        let mut resolved = PathBuf::new();
        let mut links = 0;
        while let Some(part) = pending.pop_front() {
            if part == "/" {
                resolved.clear();
                continue;
            }
            if part == "." {
                continue;
            }
            if part == ".." {
                resolved.pop();
                continue;
            }
            resolved.push(part);
            let Some(metadata) = self.observe(&resolved)? else {
                continue;
            };
            if metadata.is_symlink() && (follow_final || !pending.is_empty()) {
                links += 1;
                if links > 40 {
                    return Err(CopyError::Invalid("too many destination symlinks"));
                }
                let (root, path) = self.location(&resolved);
                let target = fs::readlinkat(root, path, Vec::new())?;
                resolved.pop();
                let bytes = target.as_bytes();
                if bytes.ends_with(b"/") || bytes.ends_with(b"/.") {
                    pending.push_front(".".into());
                }
                for component in Path::new(OsStr::from_bytes(bytes)).iter().rev() {
                    pending.push_front(component.to_owned());
                }
            } else if !pending.is_empty() && !metadata.is_dir() {
                return Err(CopyError::Invalid("destination traverses a non-directory"));
            }
        }
        Ok(resolved)
    }
    fn directory(&self, path: &Path, policy: &MetadataPolicy<'_>) -> Result<File, CopyError> {
        let mut parent = self.open(Path::new(""), OFlags::RDONLY | OFlags::DIRECTORY)?;
        let mut prefix = PathBuf::new();
        for name in path.iter() {
            prefix.push(name);
            if self.output.is_some() && prefix == Path::new("mnt") {
                parent = self.open(&prefix, OFlags::RDONLY | OFlags::DIRECTORY)?;
                continue;
            }
            let created = match fs::mkdirat(&parent, name, Mode::from_raw_mode(0o700)) {
                Ok(()) => true,
                Err(rustix::io::Errno::EXIST) => false,
                Err(error) => return Err(error.into()),
            };
            let next: File = fs::openat2(
                &parent,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
            )?
            .into();
            if created {
                policy.file(&next, 0o755, true)?;
            }
            parent = next;
        }
        Ok(parent)
    }
    fn account(&self, path: &str, name: &str, field: usize) -> Result<u32, CopyError> {
        let resolved = self.resolve(Path::new(path), true)?;
        if !self.observe(&resolved)?.is_some_and(|m| m.is_file()) {
            return Err(CopyError::Invalid("account database is not a regular file"));
        }
        let file = self.open(&resolved, OFlags::RDONLY | OFlags::NONBLOCK)?;
        const LIMIT: u64 = 16 * 1024 * 1024;
        let mut text = String::new();
        file.take(LIMIT + 1).read_to_string(&mut text)?;
        if text.len() as u64 > LIMIT {
            return Err(CopyError::Invalid("account database exceeds size limit"));
        }
        text.lines()
            .find(|line| line.split(':').next() == Some(name))
            .and_then(|line| line.split(':').nth(field))
            .and_then(|v| v.parse().ok())
            .filter(|id| *id != u32::MAX)
            .ok_or(CopyError::Invalid("missing account or invalid account ID"))
    }
    fn policy<'p>(&self, directive: &'p Copy) -> Result<MetadataPolicy<'p>, CopyError> {
        let (uid, primary) = match directive.owner() {
            None => (0, 0),
            Some(s) if s.bytes().all(|b| b.is_ascii_digit()) => {
                let id = Self::id(s)?;
                (id, id)
            }
            Some(name) => (
                self.account("/etc/passwd", name, 2)?,
                self.account("/etc/passwd", name, 3)?,
            ),
        };
        let gid = match directive.group() {
            None => primary,
            Some(s) if s.bytes().all(|b| b.is_ascii_digit()) => Self::id(s)?,
            Some(name) => self.account("/etc/group", name, 2)?,
        };
        Ok(MetadataPolicy {
            uid,
            gid,
            mode: directive.mode(),
        })
    }
    fn id(value: &str) -> Result<u32, CopyError> {
        value
            .parse()
            .ok()
            .filter(|id| *id != u32::MAX)
            .ok_or(CopyError::Invalid("invalid numeric owner/group"))
    }
    fn leaf(
        &self,
        target: &Path,
        entry: &ContextEntry,
        policy: &MetadataPolicy<'_>,
        context: &CapturedContext,
    ) -> Result<(), CopyError> {
        let name = target
            .file_name()
            .ok_or(CopyError::Invalid("file destination is image root"))?;
        if self.observe(target)?.is_some_and(|m| m.is_dir()) {
            return Err(CopyError::Invalid("file conflicts with directory"));
        }
        let parent = self.directory(target.parent().unwrap(), policy)?;
        match fs::unlinkat(&parent, name, AtFlags::empty()) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => (),
            Err(error) => return Err(error.into()),
        }
        match entry {
            ContextEntry::File { oid, executable } => {
                let bytes = context.blob(*oid)?;
                let mut file: File = fs::openat(
                    &parent,
                    name,
                    OFlags::WRONLY
                        | OFlags::CREATE
                        | OFlags::EXCL
                        | OFlags::CLOEXEC
                        | OFlags::NOFOLLOW,
                    Mode::from_raw_mode(0o600),
                )?
                .into();
                file.write_all(&bytes)?;
                policy.file(&file, if *executable { 0o755 } else { 0o644 }, false)?;
            }
            ContextEntry::Symlink(bytes) => {
                fs::symlinkat(OsStr::from_bytes(bytes), &parent, name)?;
                policy.link(&parent, name)?;
            }
            ContextEntry::Directory => {
                return Err(CopyError::Invalid("directory passed to leaf writer"));
            }
        }
        Ok(())
    }
    /// Transfer captured regular bytes through the same confined destination
    /// resolver as context COPY. No source filesystem handle is exposed.
    pub(crate) fn artifact(
        &self,
        directive: &Copy,
        source: &crate::firmware::FirmwareFile<'_>,
    ) -> Result<(), CopyError> {
        let policy = MetadataPolicy {
            uid: 0,
            gid: 0,
            mode: None,
        };
        let mut target = self.resolve(Path::new(directive.destination()), true)?;
        if directive.destination().ends_with('/') {
            self.directory(&target, &policy)?;
        }
        if self.observe(&target)?.is_some_and(|m| m.is_dir()) {
            target.push(directive.artifact());
        }
        let name = target
            .file_name()
            .ok_or(CopyError::Invalid("artifact destination is root"))?;
        let parent = self.directory(target.parent().unwrap(), &policy)?;
        match fs::unlinkat(&parent, name, AtFlags::empty()) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => (),
            Err(e) => return Err(e.into()),
        }
        let mut file: File = fs::openat(
            &parent,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(0o600),
        )?
        .into();
        io::copy(&mut source.reader(), &mut file)?;
        policy.file(&file, source.mode(), false)?;
        Ok(())
    }
    pub(crate) fn copy(
        &self,
        context: &CapturedContext,
        directive: &Copy,
    ) -> Result<(), CopyError> {
        let sources = CopySources::select(context, directive.sources())?;
        let policy = self.policy(directive)?;
        let destination = directive.destination();
        if sources.entries().len() > 1 && !destination.ends_with('/') {
            return Err(CopyError::Invalid(
                "multiple sources require trailing slash",
            ));
        }
        let target = self.resolve(Path::new(destination), true)?;
        if destination.ends_with('/') {
            self.directory(&target, &policy)?;
        }
        for source in sources.entries() {
            if source.is_directory() {
                let existed = self.observe(&target)?.is_some();
                self.directory(&target, &policy)?;
                let mut directories = Vec::new();
                for (relative, entry) in source.descendants() {
                    let path = self.resolve(&target.join(relative), false)?;
                    match entry {
                        ContextEntry::Directory => {
                            self.directory(&path, &policy)?;
                            directories.push(path);
                        }
                        _ => self.leaf(&path, entry, &policy, context)?,
                    }
                }
                for path in directories.into_iter().rev() {
                    policy.file(
                        &self.open(&path, OFlags::RDONLY | OFlags::DIRECTORY)?,
                        0o755,
                        true,
                    )?;
                }
                if !existed {
                    policy.file(
                        &self.open(&target, OFlags::RDONLY | OFlags::DIRECTORY)?,
                        0o755,
                        true,
                    )?;
                }
            } else {
                let path = if self.observe(&target)?.is_some_and(|m| m.is_dir()) {
                    target.join(
                        source
                            .selected()
                            .file_name()
                            .ok_or(CopyError::Invalid("source has no filename"))?,
                    )
                } else {
                    target.clone()
                };
                self.leaf(
                    &path,
                    source
                        .entry()
                        .ok_or(CopyError::Invalid("missing source entry"))?,
                    &policy,
                    context,
                )?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{path::ContextDirectory, request::CommitIdentity};
    use std::{
        fs as host,
        os::unix::fs::{MetadataExt, symlink},
        process::Command,
    };
    struct Fixture {
        _repo: tempfile::TempDir,
        target: tempfile::TempDir,
        context: CapturedContext,
    }
    impl Fixture {
        fn git(path: &Path, args: &[&str]) -> String {
            let output = Command::new("git")
                .args([
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "user.name=Fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-C",
                ])
                .arg(path)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        }
        fn new() -> Self {
            let repo = tempfile::tempdir().unwrap();
            host::create_dir(repo.path().join("dir")).unwrap();
            host::write(repo.path().join("dir/file"), b"committed").unwrap();
            symlink("/outside", repo.path().join("dir/link")).unwrap();
            Self::git(repo.path(), &["init", "--quiet"]);
            Self::git(repo.path(), &["add", "."]);
            Self::git(repo.path(), &["commit", "--quiet", "-m", "fixture"]);
            let commit = Self::git(repo.path(), &["rev-parse", "HEAD"]);
            let context = CapturedContext::capture(
                repo.path(),
                &CommitIdentity::try_from(commit.trim()).unwrap(),
                &ContextDirectory::try_from(Path::new(".")).unwrap(),
            )
            .unwrap();
            host::write(repo.path().join("dir/file"), b"uncommitted").unwrap();
            Self {
                _repo: repo,
                target: tempfile::tempdir().unwrap(),
                context,
            }
        }
        fn copy(&self, source: &str, destination: &str, extra: &str) -> Result<(), CopyError> {
            self.copy_layout(source, destination, extra, None)
        }
        fn copy_layout(
            &self,
            source: &str,
            destination: &str,
            extra: &str,
            output: Option<&File>,
        ) -> Result<(), CopyError> {
            let mode = if extra.trim().is_empty() {
                None
            } else {
                let values: std::collections::BTreeMap<String, String> =
                    crate::document::YamlDocument::from(extra).decode().unwrap();
                values.get("mode").cloned()
            };
            let directive = crate::recipe::Copy::fixture(
                source,
                destination,
                rustix::process::geteuid().as_raw().to_string(),
                rustix::process::getegid().as_raw().to_string(),
                mode,
            );
            let root = File::open(self.target.path()).unwrap();
            let writer = match output {
                Some(output) => CopyDestination::bootstrap(&root, output),
                None => CopyDestination::new(&root),
            };
            writer.copy(&self.context, &directive)
        }
    }
    #[test]
    fn copies_committed_contents_and_preserves_nested_links() {
        let fixture = Fixture::new();
        fixture.copy("dir", "/output", "").unwrap();
        assert_eq!(
            host::read(fixture.target.path().join("output/file")).unwrap(),
            b"committed"
        );
        assert_eq!(
            host::read_link(fixture.target.path().join("output/link")).unwrap(),
            Path::new("/outside")
        );
        fixture
            .copy("dir/file", "/mode", "        mode: 'u=rw,go='\n")
            .unwrap();
        assert_eq!(
            host::metadata(fixture.target.path().join("mode"))
                .unwrap()
                .mode()
                & 0o7777,
            0o600
        );
    }
    #[test]
    fn confines_absolute_links_and_replaces_nested_links_and_hardlinks() {
        let fixture = Fixture::new();
        host::create_dir(fixture.target.path().join("safe")).unwrap();
        symlink("/safe", fixture.target.path().join("alias")).unwrap();
        fixture.copy("dir/file", "/alias/file", "").unwrap();
        assert_eq!(
            host::read(fixture.target.path().join("safe/file")).unwrap(),
            b"committed"
        );
        let outside = tempfile::tempdir().unwrap();
        host::write(outside.path().join("sentinel"), b"untouched").unwrap();
        host::create_dir(fixture.target.path().join("out")).unwrap();
        symlink(
            outside.path().join("sentinel"),
            fixture.target.path().join("out/file"),
        )
        .unwrap();
        fixture.copy("dir", "/out", "").unwrap();
        assert_eq!(
            host::read(outside.path().join("sentinel")).unwrap(),
            b"untouched"
        );
        host::hard_link(
            outside.path().join("sentinel"),
            fixture.target.path().join("hard"),
        )
        .unwrap();
        fixture.copy("dir/file", "/hard", "").unwrap();
        assert_eq!(
            host::read(outside.path().join("sentinel")).unwrap(),
            b"untouched"
        );
        assert_eq!(
            host::read(fixture.target.path().join("hard")).unwrap(),
            b"committed"
        );
    }
    #[test]
    fn bootstrap_paths_and_links_cross_the_virtual_mount_boundary() {
        let fixture = Fixture::new();
        let output = tempfile::tempdir().unwrap();
        let output_fd = File::open(output.path()).unwrap();
        host::create_dir(fixture.target.path().join("mnt")).unwrap();
        host::write(fixture.target.path().join("mnt/hidden"), "tool backing").unwrap();
        host::create_dir(fixture.target.path().join("tool")).unwrap();
        symlink("/mnt", fixture.target.path().join("export")).unwrap();
        symlink("/tool", output.path().join("absolute-return")).unwrap();
        symlink("../tool", output.path().join("relative-return")).unwrap();
        for path in [
            "/export/forward",
            "/mnt/direct",
            "/mnt/absolute-return/a",
            "/mnt/relative-return/b",
            "/mntish/c",
        ] {
            fixture
                .copy_layout("dir/file", path, "", Some(&output_fd))
                .unwrap();
        }
        for path in ["forward", "direct"] {
            assert_eq!(host::read(output.path().join(path)).unwrap(), b"committed");
        }
        for path in ["tool/a", "tool/b", "mntish/c"] {
            assert_eq!(
                host::read(fixture.target.path().join(path)).unwrap(),
                b"committed"
            );
        }
        assert_eq!(
            host::read_dir(fixture.target.path().join("mnt"))
                .unwrap()
                .count(),
            1
        );
        assert!(!output.path().join("hidden").exists());
    }
    #[test]
    fn rejects_conflicts_and_resolves_accounts_only_inside_root() {
        let fixture = Fixture::new();
        host::write(fixture.target.path().join("conflict"), b"original").unwrap();
        assert!(fixture.copy("dir", "/conflict", "").is_err());
        assert_eq!(
            host::read(fixture.target.path().join("conflict")).unwrap(),
            b"original"
        );
        symlink("cycle", fixture.target.path().join("cycle")).unwrap();
        assert!(fixture.copy("dir/file", "/cycle", "").is_err());
        host::create_dir(fixture.target.path().join("etc")).unwrap();
        host::write(
            fixture.target.path().join("etc/passwd"),
            "builder:x:123:456::/:/bin/sh\n",
        )
        .unwrap();
        let root = File::open(fixture.target.path()).unwrap();
        let writer = CopyDestination::new(&root);
        assert_eq!(writer.account("/etc/passwd", "builder", 2).unwrap(), 123);
        assert_eq!(writer.account("/etc/passwd", "builder", 3).unwrap(), 456);
        assert!(writer.account("/etc/passwd", "absent", 2).is_err());
        assert!(CopyDestination::id("4294967295").is_err());
        host::remove_file(fixture.target.path().join("etc/passwd")).unwrap();
        fs::mknodat(
            &root,
            "etc/passwd",
            fs::FileType::Fifo,
            Mode::from_raw_mode(0o600),
            0,
        )
        .unwrap();
        assert!(writer.account("/etc/passwd", "builder", 2).is_err());
    }
}
