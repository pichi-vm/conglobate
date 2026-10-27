// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
//! COPY selection from immutable Git objects; no ambient filesystem traversal.
use super::SourcePattern;
use crate::context::{CapturedContext, ContextEntry};
use std::{
    collections::{BTreeMap, VecDeque},
    ffi::{OsStr, OsString},
    ops::Bound::{Included, Unbounded},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub(crate) enum SelectionError {
    #[error("COPY source matched no entries: {0}")]
    NoMatch(String),
    #[error("COPY source is missing or traverses a non-directory: {0:?}")]
    Missing(PathBuf),
    #[error("COPY source symlink escapes the context or has a cycle: {0:?}")]
    Symlink(PathBuf),
    #[error(transparent)]
    Pattern(#[from] super::PatternError),
}
/// A selected name and its resolved immutable object. Directory descendants are
/// returned without following nested symlinks; a writer must preserve those links.
#[derive(Debug)]
pub(crate) struct CopySource<'a> {
    selected: PathBuf,
    resolved: PathBuf,
    tree: SourceTree<'a>,
}
impl CopySource<'_> {
    #[cfg(test)]
    pub(crate) fn resolved(&self) -> &Path {
        &self.resolved
    }
    pub(crate) fn selected(&self) -> &Path {
        &self.selected
    }
    /// None denotes the context root directory, which has no Git entry of its own.
    pub(crate) fn entry(&self) -> Option<&ContextEntry> {
        self.tree.entries.get(&self.resolved)
    }
    pub(crate) fn is_directory(&self) -> bool {
        self.resolved.as_os_str().is_empty()
            || matches!(self.entry(), Some(ContextEntry::Directory))
    }
    pub(crate) fn descendants(&self) -> impl Iterator<Item = (&Path, &ContextEntry)> {
        let directory = self.is_directory();
        self.tree
            .entries
            .range::<Path, _>((Included(self.resolved.as_path()), Unbounded))
            .take_while(move |(path, _)| directory && path.starts_with(&self.resolved))
            .filter_map(|(path, entry)| {
                let relative = path.strip_prefix(&self.resolved).ok()?;
                (!relative.as_os_str().is_empty()).then_some((relative, entry))
            })
    }
}
#[derive(Debug)]
pub(crate) struct CopySources<'a> {
    entries: Vec<CopySource<'a>>,
}
impl<'a> CopySources<'a> {
    pub(crate) fn select(
        context: &'a CapturedContext,
        patterns: &[SourcePattern],
    ) -> Result<Self, SelectionError> {
        SourceTree {
            entries: context.entries(),
        }
        .select(patterns)
    }
    pub(crate) fn entries(&self) -> &[CopySource<'a>] {
        &self.entries
    }
}
#[derive(Debug, Clone, Copy)]
struct SourceTree<'a> {
    entries: &'a BTreeMap<PathBuf, ContextEntry>,
}
impl<'a> SourceTree<'a> {
    fn select(self, patterns: &[SourcePattern]) -> Result<CopySources<'a>, SelectionError> {
        let mut entries = Vec::new();
        for pattern in patterns {
            let mut paths = vec![PathBuf::new()];
            let mut wildcard_seen = false;
            for part in pattern
                .source()
                .trim_matches('/')
                .split('/')
                .filter(|p| !p.is_empty() && *p != ".")
            {
                let wildcard = Self::wildcard(part);
                let matcher: SourcePattern = part.parse()?;
                let mut next = Vec::new();
                for path in paths {
                    // Before wildcard walking, an explicit directory symlink is
                    // resolved. Once walking, symlinks are leaves (fsutil semantics).
                    let actual = if wildcard_seen {
                        if !path.as_os_str().is_empty()
                            && !matches!(self.entries.get(&path), Some(ContextEntry::Directory))
                        {
                            continue;
                        }
                        path
                    } else {
                        self.resolve(&path)?
                    };
                    if !actual.as_os_str().is_empty()
                        && !matches!(self.entries.get(&actual), Some(ContextEntry::Directory))
                    {
                        continue;
                    }
                    if wildcard || wildcard_seen {
                        for child in self
                            .entries
                            .range::<Path, _>((Included(actual.as_path()), Unbounded))
                            .take_while(|(path, _)| path.starts_with(&actual))
                            .map(|(path, _)| path)
                            .filter(|p| p.parent() == Some(actual.as_path()))
                        {
                            let name = child.file_name().expect("captured entry name");
                            if matcher.matches(name) {
                                next.push(actual.join(name));
                            }
                        }
                    } else {
                        next.push(actual.join(part));
                    }
                }
                next.sort();
                paths = next;
                wildcard_seen |= wildcard;
            }
            if wildcard_seen {
                paths.retain(|p| p.as_os_str().is_empty() || self.entries.contains_key(p));
            }
            if paths.is_empty() {
                return Err(SelectionError::NoMatch(pattern.source().to_owned()));
            }
            for selected in paths {
                let resolved = self.resolve(&selected)?;
                entries.push(CopySource {
                    selected,
                    resolved,
                    tree: self,
                });
            }
        }
        if entries.is_empty() {
            return Err(SelectionError::NoMatch(String::new()));
        }
        Ok(CopySources { entries })
    }
    fn wildcard(part: &str) -> bool {
        let mut chars = part.chars();
        while let Some(ch) = chars.next() {
            match ch {
                '\\' => {
                    chars.next();
                }
                '*' | '?' | '[' => return true,
                _ => {}
            }
        }
        false
    }
    fn resolve(self, path: &Path) -> Result<PathBuf, SelectionError> {
        let mut pending: VecDeque<OsString> = path.iter().map(OsString::from).collect();
        let mut resolved = PathBuf::new();
        let mut links = 0;
        while let Some(part) = pending.pop_front() {
            if part == "." {
                continue;
            }
            if part == ".." {
                if !resolved.pop() {
                    return Err(SelectionError::Symlink(path.to_owned()));
                }
                continue;
            }
            resolved.push(part);
            match self
                .entries
                .get(&resolved)
                .ok_or_else(|| SelectionError::Missing(path.to_owned()))?
            {
                ContextEntry::Symlink(bytes) => {
                    links += 1;
                    let target = Path::new(OsStr::from_bytes(bytes));
                    if links > 40 || target.is_absolute() {
                        return Err(SelectionError::Symlink(path.to_owned()));
                    }
                    resolved.pop();
                    if bytes.ends_with(b"/") || bytes.ends_with(b"/.") {
                        pending.push_front(OsString::from("."));
                    }
                    for part in target.iter().rev() {
                        pending.push_front(part.to_owned());
                    }
                }
                ContextEntry::File { .. } if !pending.is_empty() => {
                    return Err(SelectionError::Missing(path.to_owned()));
                }
                _ => {}
            }
        }
        Ok(resolved)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(BTreeMap<PathBuf, ContextEntry>);
    impl Fixture {
        fn new() -> Self {
            Self(BTreeMap::from([
                ("dir".into(), ContextEntry::Directory),
                (
                    "dir/file".into(),
                    ContextEntry::File {
                        executable: true,
                        oid: gix::ObjectId::empty_blob(gix::hash::Kind::Sha1),
                    },
                ),
                (
                    "dir/link".into(),
                    ContextEntry::Symlink(b"../elsewhere".to_vec()),
                ),
                ("alias".into(), ContextEntry::Symlink(b"dir".to_vec())),
                (
                    "outside".into(),
                    ContextEntry::Symlink(b"../outside".to_vec()),
                ),
                (
                    "absolute".into(),
                    ContextEntry::Symlink(b"/etc/passwd".to_vec()),
                ),
                ("cycle".into(), ContextEntry::Symlink(b"cycle".to_vec())),
                (
                    "bad-dir".into(),
                    ContextEntry::Symlink(b"dir/file/".to_vec()),
                ),
                (
                    "elsewhere".into(),
                    ContextEntry::File {
                        executable: false,
                        oid: gix::ObjectId::empty_blob(gix::hash::Kind::Sha1),
                    },
                ),
            ]))
        }
        fn select(&self, patterns: &[&str]) -> Result<CopySources<'_>, SelectionError> {
            SourceTree { entries: &self.0 }.select(
                &patterns
                    .iter()
                    .map(|s| s.parse().unwrap())
                    .collect::<Vec<_>>(),
            )
        }
    }
    #[test]
    fn explicit_links_resolve_but_nested_links_are_preserved() {
        let fixture = Fixture::new();
        let sources = fixture.select(&["alias"]).unwrap();
        let source = &sources.entries()[0];
        assert_eq!(source.selected(), Path::new("alias"));
        assert_eq!(source.resolved(), Path::new("dir"));
        assert!(source.is_directory());
        assert!(
            source
                .descendants()
                .any(|(p, e)| p == Path::new("link") && matches!(e, ContextEntry::Symlink(_)))
        );
        assert_eq!(
            fixture.select(&["dir/link"]).unwrap().entries()[0].resolved(),
            Path::new("elsewhere")
        );
    }
    #[test]
    fn wildcard_walk_is_sorted_and_does_not_follow_directory_links() {
        let fixture = Fixture::new();
        let sources = fixture.select(&["*/file"]).unwrap();
        assert_eq!(sources.entries().len(), 1);
        assert_eq!(sources.entries()[0].resolved(), Path::new("dir/file"));
        let sources = fixture.select(&["alias/f*"]).unwrap();
        assert_eq!(sources.entries()[0].resolved(), Path::new("dir/file"));
        assert!(!SourceTree::wildcard(r"literal\*"));
        assert!(SourceTree::wildcard(r"literal\**"));
        assert!(fixture.select(&["absent*"]).is_err());
        assert!(fixture.select(&["dir/file", "absent*"]).is_err());
        assert!(fixture.select(&["dir/file/child"]).is_err());
    }
    #[test]
    fn wildcard_prefix_detection_and_suffix_escaping_match_fsutil() {
        let mut fixture = Fixture::new();
        for name in [r"literal\*", "dir/a?"] {
            fixture.0.insert(
                name.into(),
                ContextEntry::File {
                    executable: false,
                    oid: gix::ObjectId::empty_blob(gix::hash::Kind::Sha1),
                },
            );
        }
        assert_eq!(
            fixture.select(&[r"literal\*"]).unwrap().entries()[0].selected(),
            Path::new(r"literal\*")
        );
        assert_eq!(
            fixture.select(&[r"*/a\?"]).unwrap().entries()[0].selected(),
            Path::new("dir/a?")
        );
    }
    #[test]
    fn subtree_selection_respects_components_and_direct_child_depth() {
        let mut fixture = Fixture::new();
        for name in ["dir-x", "dir/sub", "dir/sub/deeper", "dir2"] {
            fixture.0.insert(name.into(), ContextEntry::Directory);
        }
        let sources = fixture.select(&["dir"]).unwrap();
        let descendants: Vec<_> = sources.entries()[0]
            .descendants()
            .map(|(path, _)| path.to_path_buf())
            .collect();
        assert_eq!(
            descendants,
            ["file", "link", "sub", "sub/deeper"].map(PathBuf::from)
        );
        let children = fixture.select(&["dir/*"]).unwrap();
        assert_eq!(
            children
                .entries()
                .iter()
                .map(CopySource::selected)
                .collect::<Vec<_>>(),
            [
                Path::new("dir/file"),
                Path::new("dir/link"),
                Path::new("dir/sub")
            ]
        );
        let file = fixture.select(&["dir/file"]).unwrap();
        assert_eq!(file.entries()[0].descendants().count(), 0);
        let root = fixture.select(&["/"]).unwrap();
        assert_eq!(root.entries()[0].descendants().count(), fixture.0.len());
    }
    #[test]
    fn root_selection_and_symlink_confinement() {
        let fixture = Fixture::new();
        assert!(fixture.select(&["/"]).unwrap().entries()[0].is_directory());
        for path in ["outside", "absolute", "cycle", "bad-dir"] {
            assert!(fixture.select(&[path]).is_err(), "{path}");
        }
        assert_eq!(
            fixture
                .select(&["dir/file", "dir/file"])
                .unwrap()
                .entries()
                .len(),
            2
        );
    }
}
