// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::storage::FailureDiagnostics;
use rustix::fs::{self, AtFlags, Mode, OFlags, ResolveFlags};
use std::{
    collections::VecDeque,
    ffi::OsString,
    fs::File,
    io,
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
    path::{Component, Path, PathBuf},
};

/// A regular resolver target selected inside the exclusively owned command
/// root. Existing symlinks are followed inside that root; missing ancestors and
/// the final placeholder are owned until explicit cleanup. No file is activated
/// for reading/writing merely to inspect its type. This object is not a mount.
/// Commands must be quiescent before resolution and cleanup. Abandonment never
/// removes image paths; a failed build must discard its writable layer.
#[derive(Debug)]
pub(crate) struct ResolverTarget {
    root: File,
    target: File,
    namespace: File,
    path: PathBuf,
    scaffold: ResolverScaffold,
}
#[derive(Debug, thiserror::Error)]
#[error("resolver target: {diagnostics}")]
pub(crate) struct ResolverTargetFailure {
    diagnostics: FailureDiagnostics,
    scaffold: ResolverScaffold,
}
impl ResolverTargetFailure {
    pub(crate) fn cleanup(self) -> Result<(), Self> {
        let Self {
            diagnostics,
            scaffold,
        } = self;
        scaffold.release().map_err(|failure| Self {
            diagnostics: diagnostics.followed_by(failure.diagnostics),
            scaffold: failure.scaffold,
        })
    }
}
#[derive(Debug, Default)]
struct ResolverScaffold {
    entries: Vec<CreatedEntry>,
}
#[derive(Debug)]
struct CreatedEntry {
    parent: File,
    node: CreatedNode,
    name: OsString,
    kind: CreatedKind,
}
#[derive(Debug)]
enum CreatedNode {
    Pinned(File),
    UnpinnedDirectory,
}
#[derive(Debug)]
enum CreatedKind {
    Directory,
    File,
}
impl CreatedEntry {
    fn remove(&self) -> io::Result<()> {
        let actual = fs::statat(&self.parent, &self.name, AtFlags::SYMLINK_NOFOLLOW)?;
        let expected = match &self.node {
            CreatedNode::Pinned(node) => node.metadata()?,
            CreatedNode::UnpinnedDirectory => {
                return Err(io::Error::other(
                    "cannot safely remove unpinned resolver directory; discard build layer",
                ));
            }
        };
        if (actual.st_dev, actual.st_ino) != (expected.dev(), expected.ino()) {
            return Err(io::Error::other("resolver scaffold entry was replaced"));
        }
        let flags = match self.kind {
            CreatedKind::Directory => AtFlags::REMOVEDIR,
            CreatedKind::File => AtFlags::empty(),
        };
        fs::unlinkat(&self.parent, &self.name, flags)?;
        Ok(())
    }
}
impl ResolverScaffold {
    fn release(mut self) -> Result<(), ResolverTargetFailure> {
        while let Some(entry) = self.entries.last() {
            if let Err(source) = entry.remove() {
                return Err(ResolverTargetFailure {
                    diagnostics: source.into(),
                    scaffold: self,
                });
            }
            self.entries.pop();
        }
        Ok(())
    }
    fn create(&mut self, parent: File, name: OsString, directory: bool) -> io::Result<File> {
        // Reserve bookkeeping before creating anything whose cleanup must be
        // retained. The exclusive root has no admitted image writers here.
        self.entries.try_reserve(1).map_err(io::Error::other)?;
        let node = if directory {
            fs::mkdirat(&parent, &name, Mode::from_raw_mode(0o755))?;
            // If pinning fails after mkdir, retain the created path as an
            // unresolved obligation. Cleanup must not claim it was removed.
            match fs::openat2(
                &parent,
                &name,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_XDEV | ResolveFlags::NO_MAGICLINKS,
            ) {
                Ok(node) => File::from(node),
                Err(error) => {
                    self.entries.push(CreatedEntry {
                        parent,
                        node: CreatedNode::UnpinnedDirectory,
                        name,
                        kind: CreatedKind::Directory,
                    });
                    return Err(error.into());
                }
            }
        } else {
            File::from(fs::openat(
                &parent,
                &name,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )?)
        };
        let result = node.try_clone();
        self.entries.push(CreatedEntry {
            parent,
            node: CreatedNode::Pinned(node),
            name,
            kind: if directory {
                CreatedKind::Directory
            } else {
                CreatedKind::File
            },
        });
        result
    }
}
impl ResolverTarget {
    pub(crate) fn resolve(root: File) -> Result<Self, ResolverTargetFailure> {
        Self::select(root, true)
    }
    /// Command setup never creates image paths. Its enclosing owner must have
    /// prepared and retained any scaffolding before handing off the root.
    pub(crate) fn existing(root: File) -> Result<Self, ResolverTargetFailure> {
        Self::select(root, false)
    }
    fn select(root: File, create: bool) -> Result<Self, ResolverTargetFailure> {
        let mut scaffold = ResolverScaffold::default();
        let namespace =
            File::open("/proc/self/ns/mnt").map_err(|source| ResolverTargetFailure {
                diagnostics: source.into(),
                scaffold: ResolverScaffold::default(),
            })?;
        match Self::walk(&root, &mut scaffold, create) {
            Ok((target, path)) => Ok(Self {
                root,
                target,
                namespace,
                path,
                scaffold,
            }),
            Err(source) => Err(ResolverTargetFailure {
                diagnostics: source.into(),
                scaffold,
            }),
        }
    }
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn metadata(&self) -> io::Result<std::fs::Metadata> {
        self.target.metadata()
    }
    pub(super) fn mount_target(&self) -> io::Result<&File> {
        let current = File::open("/proc/self/ns/mnt")?.metadata()?;
        let selected = self.namespace.metadata()?;
        if (current.dev(), current.ino()) != (selected.dev(), selected.ino()) {
            return Err(io::Error::other(
                "resolver target belongs to another mount namespace",
            ));
        }
        Ok(&self.target)
    }
    pub(crate) fn release(self) -> Result<(), ResolverTargetFailure> {
        let Self {
            root,
            target,
            scaffold,
            ..
        } = self;
        drop(target);
        // Keep the root pinned until all owned entries have been processed.
        let result = scaffold.release();
        drop(root);
        result
    }
    fn components(path: &Path) -> VecDeque<OsString> {
        path.components()
            .map(|part| part.as_os_str().to_owned())
            .collect()
    }
    fn walk(
        root: &File,
        scaffold: &mut ResolverScaffold,
        create: bool,
    ) -> io::Result<(File, PathBuf)> {
        if !root.metadata()?.is_dir() {
            return Err(io::Error::other("resolver root is not a directory"));
        }
        let mut forbidden = Vec::new();
        for name in ["dev", "proc", "sys"] {
            match fs::openat2(
                root,
                name,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS,
            ) {
                Ok(directory) => {
                    let metadata = File::from(directory).metadata()?;
                    forbidden.push((metadata.dev(), metadata.ino()));
                }
                Err(rustix::io::Errno::NOENT) => (),
                Err(error) => return Err(error.into()),
            }
        }
        let mut pending = Self::components(Path::new("etc/resolv.conf"));
        let mut resolved = PathBuf::new();
        let mut links = 0;
        for _ in 0..4096 {
            let Some(part) = pending.pop_front() else {
                return Err(io::Error::other("resolver target is a directory"));
            };
            match Path::new(&part)
                .components()
                .next()
                .expect("nonempty component")
            {
                Component::RootDir => {
                    resolved.clear();
                    continue;
                }
                Component::CurDir => continue,
                Component::ParentDir => {
                    if !resolved.pop() {
                        return Err(io::Error::other("resolver symlink escapes command root"));
                    }
                    continue;
                }
                Component::Normal(_) => (),
                Component::Prefix(_) => {
                    return Err(io::Error::other("invalid resolver path prefix"));
                }
            }
            let parent = File::from(fs::openat2(
                root,
                if resolved.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    &resolved
                },
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_XDEV,
            )?);
            let node = match fs::openat2(
                &parent,
                &part,
                OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_XDEV,
            ) {
                Ok(node) => File::from(node),
                Err(rustix::io::Errno::NOENT) if create => {
                    scaffold.create(parent, part.clone(), !pending.is_empty())?
                }
                Err(error) => return Err(error.into()),
            };
            let metadata = node.metadata()?;
            if metadata.file_type().is_symlink() {
                links += 1;
                if links > 40 {
                    return Err(io::Error::other("resolver symlink limit exceeded"));
                }
                let target = fs::readlinkat(&node, "", Vec::new())?;
                let mut expanded =
                    Self::components(Path::new(std::ffi::OsStr::from_bytes(target.to_bytes())));
                expanded.append(&mut pending);
                pending = expanded;
                continue;
            }
            resolved.push(&part);
            if resolved.components().next().is_some_and(|part| {
                ["dev", "proc", "sys"]
                    .iter()
                    .any(|name| part.as_os_str() == *name)
            }) || forbidden.contains(&(metadata.dev(), metadata.ino()))
            {
                return Err(io::Error::other(
                    "resolver target is inside a command runtime mount",
                ));
            }

            if pending.is_empty() {
                if !metadata.is_file() {
                    return Err(io::Error::other("resolver target must be a regular file"));
                }
                // The kernel re-resolves the original spelling as the final
                // authority, including trailing-slash and magic-link rules.
                let selected = File::from(fs::openat2(
                    root,
                    "etc/resolv.conf",
                    OFlags::PATH | OFlags::CLOEXEC,
                    Mode::empty(),
                    ResolveFlags::IN_ROOT | ResolveFlags::NO_MAGICLINKS | ResolveFlags::NO_XDEV,
                )?);
                let actual = selected.metadata()?;
                if (actual.dev(), actual.ino()) != (metadata.dev(), metadata.ino()) {
                    return Err(io::Error::other("resolver target changed during selection"));
                }
                return Ok((selected, resolved));
            }
            if !metadata.is_dir() {
                return Err(io::Error::other("resolver ancestor is not a directory"));
            }
        }
        Err(io::Error::other("resolver path expansion limit exceeded"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs as stdfs,
        os::unix::{fs::symlink, net::UnixListener},
    };
    #[test]
    fn existing_selection_never_creates_scaffolding() {
        let root = tempfile::tempdir().unwrap();
        ResolverTarget::existing(File::open(root.path()).unwrap())
            .unwrap_err()
            .cleanup()
            .unwrap();
        assert_eq!(stdfs::read_dir(root.path()).unwrap().count(), 0);
        stdfs::create_dir(root.path().join("etc")).unwrap();
        symlink("/run/resolver", root.path().join("etc/resolv.conf")).unwrap();
        ResolverTarget::existing(File::open(root.path()).unwrap())
            .unwrap_err()
            .cleanup()
            .unwrap();
        assert!(!root.path().join("run").exists());
    }
    #[test]
    fn dangling_absolute_symlink_is_confined_and_scaffold_removed() {
        let root = tempfile::tempdir().unwrap();
        stdfs::create_dir(root.path().join("etc")).unwrap();
        symlink(
            "/run/resolver/generated",
            root.path().join("etc/resolv.conf"),
        )
        .unwrap();
        let selected = ResolverTarget::resolve(File::open(root.path()).unwrap()).unwrap();
        assert_eq!(selected.path(), Path::new("run/resolver/generated"));
        assert!(selected.metadata().unwrap().is_file());
        selected.release().unwrap();
        assert!(!root.path().join("run").exists());
        assert_eq!(
            stdfs::read_link(root.path().join("etc/resolv.conf")).unwrap(),
            Path::new("/run/resolver/generated")
        );
    }
    #[test]
    fn existing_regular_target_and_relative_symlink_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        stdfs::create_dir(root.path().join("etc")).unwrap();
        stdfs::create_dir(root.path().join("run")).unwrap();
        stdfs::write(root.path().join("run/resolver"), "original").unwrap();
        symlink("../run/resolver", root.path().join("etc/resolv.conf")).unwrap();
        ResolverTarget::resolve(File::open(root.path()).unwrap())
            .unwrap()
            .release()
            .unwrap();
        assert_eq!(
            stdfs::read(root.path().join("run/resolver")).unwrap(),
            b"original"
        );
    }
    #[test]
    fn cleanup_refuses_replacement_and_can_retry_after_original_is_restored() {
        let root = tempfile::tempdir().unwrap();
        stdfs::create_dir(root.path().join("etc")).unwrap();
        let selected = ResolverTarget::resolve(File::open(root.path()).unwrap()).unwrap();
        let path = root.path().join("etc/resolv.conf");
        let saved = root.path().join("saved");
        stdfs::rename(&path, &saved).unwrap();
        stdfs::write(&path, b"replacement").unwrap();
        let failure = selected.release().unwrap_err();
        assert_eq!(stdfs::read(&path).unwrap(), b"replacement");
        stdfs::remove_file(&path).unwrap();
        stdfs::rename(saved, &path).unwrap();
        failure.cleanup().unwrap();
        assert!(!path.exists());
        assert!(root.path().join("etc").is_dir());
    }
    #[test]
    fn cleanup_preserves_additions_and_partial_path_failure_remains_owned() {
        let root = tempfile::tempdir().unwrap();
        let selected = ResolverTarget::resolve(File::open(root.path()).unwrap()).unwrap();
        stdfs::write(root.path().join("etc/keep"), b"keep").unwrap();
        let failure = selected.release().unwrap_err();
        assert_eq!(stdfs::read(root.path().join("etc/keep")).unwrap(), b"keep");
        stdfs::remove_file(root.path().join("etc/keep")).unwrap();
        failure.cleanup().unwrap();
        assert!(!root.path().join("etc").exists());
        stdfs::create_dir(root.path().join("etc")).unwrap();
        symlink(
            "/run/../etc/resolv.conf",
            root.path().join("etc/resolv.conf"),
        )
        .unwrap();
        let failure = ResolverTarget::resolve(File::open(root.path()).unwrap()).unwrap_err();
        assert!(root.path().join("run").is_dir());
        failure.cleanup().unwrap();
        assert!(!root.path().join("run").exists());
    }
    #[test]
    fn runtime_mounts_and_their_aliases_cannot_hide_the_resolver() {
        for runtime in ["dev", "proc", "sys"] {
            for alias in [false, true] {
                let root = tempfile::tempdir().unwrap();
                stdfs::create_dir(root.path().join("etc")).unwrap();
                let destination = if alias { "private-runtime" } else { runtime };
                stdfs::create_dir(root.path().join(destination)).unwrap();
                if alias {
                    symlink(format!("/{destination}"), root.path().join(runtime)).unwrap();
                }
                symlink(
                    format!("/{destination}/resolver"),
                    root.path().join("etc/resolv.conf"),
                )
                .unwrap();
                let failure =
                    ResolverTarget::resolve(File::open(root.path()).unwrap()).unwrap_err();
                assert!(failure.to_string().contains("runtime mount"));
                failure.cleanup().unwrap();
                assert!(!root.path().join(destination).join("resolver").exists());
            }
        }
    }
    #[test]
    fn cleanup_uses_pinned_parent_after_parent_path_replacement() {
        let root = tempfile::tempdir().unwrap();
        stdfs::create_dir(root.path().join("etc")).unwrap();
        let selected = ResolverTarget::resolve(File::open(root.path()).unwrap()).unwrap();
        stdfs::rename(root.path().join("etc"), root.path().join("saved")).unwrap();
        stdfs::create_dir(root.path().join("etc")).unwrap();
        stdfs::write(root.path().join("etc/resolv.conf"), b"image replacement").unwrap();
        selected.release().unwrap();
        assert!(!root.path().join("saved/resolv.conf").exists());
        assert_eq!(
            stdfs::read(root.path().join("etc/resolv.conf")).unwrap(),
            b"image replacement"
        );
    }
    #[test]
    fn rejects_escape_directory_fifo_socket_and_trailing_slash_without_activation() {
        for link in [
            "../../escape",
            "/etc/resolv.conf",
            "/regular/",
            "/regular/.",
        ] {
            let root = tempfile::tempdir().unwrap();
            stdfs::create_dir(root.path().join("etc")).unwrap();
            stdfs::write(root.path().join("regular"), b"original").unwrap();
            symlink(link, root.path().join("etc/resolv.conf")).unwrap();
            ResolverTarget::resolve(File::open(root.path()).unwrap())
                .unwrap_err()
                .cleanup()
                .unwrap();
            assert_eq!(
                stdfs::read(root.path().join("regular")).unwrap(),
                b"original"
            );
        }
        for kind in [0, 1, 2] {
            let root = tempfile::tempdir().unwrap();
            stdfs::create_dir(root.path().join("etc")).unwrap();
            let path = root.path().join("etc/resolv.conf");
            let _socket = match kind {
                0 => {
                    stdfs::create_dir(&path).unwrap();
                    None
                }
                1 => {
                    fs::mknodat(
                        fs::CWD,
                        &path,
                        fs::FileType::Fifo,
                        Mode::from_raw_mode(0o600),
                        0,
                    )
                    .unwrap();
                    None
                }
                _ => Some(UnixListener::bind(&path).unwrap()),
            };
            ResolverTarget::resolve(File::open(root.path()).unwrap())
                .unwrap_err()
                .cleanup()
                .unwrap();
            assert!(path.symlink_metadata().is_ok());
        }
    }
}
