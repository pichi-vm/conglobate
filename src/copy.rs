// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Context-to-image COPY: top-level symlinks are followed, nested symlinks
//! preserved, directories merged by contents, and paths confined to their tree.
//! Semantics reference: Dockerfile COPY and tonistiigi/fsutil's copy package.

use crate::{Error, paths::resolve, recipe::Copy, template::Template};
use nix::fcntl::AtFlags;
use nix::unistd::{Gid, Uid, fchownat};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

impl Copy {
    pub fn apply(&self, context: &Path, root: &Path, carapace: Option<&str>) -> Result<(), Error> {
        let expand = |text: &str| Template::parse(text, None)?.expand(carapace, None);
        let destination = expand(self.into.as_ref())?;
        let target = resolve(root, Path::new(&destination), true)?;
        let owner = self.owner.as_deref().map(expand).transpose()?;
        let group = self.group.as_deref().map(expand).transpose()?;
        let (uid, gid) = ownership(root, owner.as_deref(), group.as_deref())?;
        let mode = self.mode.as_deref().map(expand).transpose()?;
        let mut sources = Vec::new();
        for source in self.from.paths() {
            sources.extend(expand_sources(context, &expand(source)?)?);
        }
        if sources.len() > 1 && !destination.ends_with('/') {
            return Err(Error::Invalid(
                "multiple COPY sources require an into path ending in /".into(),
            ));
        }
        let policy = Policy {
            uid,
            gid,
            mode: mode.as_deref(),
        };
        if destination.ends_with('/') {
            policy.parents(&target)?;
        }
        for source in sources {
            let actual = resolve(context, &source, true)?;
            let metadata = fs::metadata(&actual)?;
            if metadata.is_dir() {
                // COPY merges directory contents into a destination prepared
                // with default metadata, regardless of a trailing slash.
                policy.parents(&target)?;
            }
            let appended = metadata.is_file() && target.is_dir();
            let target = if appended {
                target.join(
                    source
                        .file_name()
                        .ok_or_else(|| Error::Invalid("COPY source has no filename".into()))?,
                )
            } else {
                target.clone()
            };
            policy.copy(&actual, &target, root, !appended)?;
        }
        Ok(())
    }
}

fn expand_sources(context: &Path, source: &str) -> Result<Vec<PathBuf>, Error> {
    let mut paths = vec![PathBuf::new()];
    let mut walked = false;
    for component in source
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
    {
        if component == ".." {
            return Err(Error::Invalid("COPY source traverses its context".into()));
        }
        // Match one path component at a time, so ** has no recursive meaning.
        let pattern = crate::copy_pattern::Pattern::parse(component)?;
        let wildcard = component.contains(['*', '?', '[', '\\']);
        let mut next = Vec::new();
        for path in paths {
            if walked {
                // Docker's wildcard walk does not descend through symlinks or
                // files encountered while walking the wildcard suffix.
                let candidate = resolve(context, &path, false)?;
                match fs::symlink_metadata(candidate) {
                    Ok(metadata) if metadata.is_dir() => {}
                    Ok(_) => continue,
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                        ) =>
                    {
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                }
            }
            if wildcard {
                let directory = resolve(context, &path, true)?;
                for entry in fs::read_dir(directory)? {
                    let name = entry?.file_name();
                    if pattern.matches(&name) {
                        next.push(path.join(name));
                    }
                }
            } else {
                next.push(path.join(component));
            }
        }
        next.sort();
        paths = next;
        walked |= wildcard;
    }
    if walked {
        let mut existing = Vec::new();
        for path in paths {
            match resolve(context, &path, false)?.symlink_metadata() {
                Ok(_) => existing.push(path),
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                    ) => {}
                Err(error) => return Err(error.into()),
            }
        }
        paths = existing;
    }
    if paths.is_empty() {
        return Err(Error::Invalid(format!(
            "COPY source {source:?} matched no files"
        )));
    }
    Ok(paths)
}

fn account(root: &Path, path: &str, name: &str) -> Result<Vec<String>, Error> {
    let text = fs::read_to_string(resolve(root, Path::new(path), true)?)?;
    text.lines()
        .find(|line| line.split(':').next() == Some(name))
        .map(|line| line.split(':').map(str::to_owned).collect())
        .ok_or_else(|| Error::Invalid(format!("COPY account {name:?} not found in {path}")))
}

fn account_id(fields: &[String], field: usize) -> Result<u32, Error> {
    fields
        .get(field)
        .and_then(|id| id.parse().ok())
        .ok_or_else(|| Error::Invalid("invalid COPY account id".into()))
}

fn ownership(root: &Path, owner: Option<&str>, group: Option<&str>) -> Result<(u32, u32), Error> {
    // Named users inherit their passwd primary GID; numeric owners use UID=GID.
    let (uid, mut gid) = match owner {
        None => (0, 0),
        Some(value) => {
            if let Ok(id) = value.parse::<u32>() {
                (id, id)
            } else {
                let fields = account(root, "/etc/passwd", value)?;
                (account_id(&fields, 2)?, account_id(&fields, 3)?)
            }
        }
    };
    if let Some(group) = group {
        gid = match group.parse::<u32>() {
            Ok(id) => id,
            Err(_) => account_id(&account(root, "/etc/group", group)?, 2)?,
        };
    }
    Ok((uid, gid))
}

struct Policy<'a> {
    uid: u32,
    gid: u32,
    mode: Option<&'a str>,
}

impl Policy<'_> {
    fn parents(&self, path: &Path) -> Result<(), Error> {
        if path.is_dir() {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            self.parents(parent)?;
        }
        fs::create_dir(path)?;
        self.ownership(path)?;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(self.permissions(0o755, true)?),
        )?;
        Ok(())
    }

    fn ownership(&self, path: &Path) -> Result<(), Error> {
        fchownat(
            nix::fcntl::AT_FDCWD,
            path,
            Some(Uid::from_raw(self.uid)),
            Some(Gid::from_raw(self.gid)),
            AtFlags::AT_SYMLINK_NOFOLLOW,
        )?;
        Ok(())
    }

    fn copy(&self, source: &Path, target: &Path, root: &Path, top: bool) -> Result<(), Error> {
        let metadata = fs::symlink_metadata(source)?;
        let relative = target
            .strip_prefix(root)
            .map_err(|_| Error::Invalid("COPY target escaped root".into()))?;
        // Docker resolves the declared destination, but replaces nested symlinks
        // when copying over them rather than writing through their referents.
        let target = resolve(root, relative, top)?;
        let existing = fs::symlink_metadata(&target).ok();
        if metadata.is_dir() {
            if existing.as_ref().is_some_and(|m| !m.is_dir()) {
                return Err(Error::Invalid(
                    "COPY directory conflicts with a non-directory".into(),
                ));
            }
            self.parents(&target)?;
            let mut children: Vec<_> = fs::read_dir(source)?.collect::<Result<_, _>>()?;
            children.sort_by_key(std::fs::DirEntry::file_name);
            for entry in children {
                self.copy(&entry.path(), &target.join(entry.file_name()), root, false)?;
            }
            if top && existing.is_some() {
                return Ok(());
            }
        } else {
            if existing.as_ref().is_some_and(std::fs::Metadata::is_dir) {
                return Err(Error::Invalid(
                    "COPY file conflicts with a directory".into(),
                ));
            }
            if let Some(parent) = target.parent() {
                self.parents(parent)?;
            }
            if existing.is_some() {
                fs::remove_file(&target)?;
            }
            if metadata.is_symlink() {
                std::os::unix::fs::symlink(fs::read_link(source)?, &target)?;
            } else if metadata.is_file() {
                fs::copy(source, &target)?;
            } else {
                return Err(Error::Invalid("unsupported COPY source file type".into()));
            }
        }
        self.ownership(&target)?;
        if !metadata.is_symlink() {
            fs::set_permissions(
                &target,
                fs::Permissions::from_mode(
                    self.permissions(metadata.mode() & 0o7777, metadata.is_dir())?,
                ),
            )?;
        }
        for name in xattr::list(source)? {
            if let Some(value) = xattr::get(source, &name)? {
                xattr::set(&target, name, &value)?;
            }
        }
        filetime::set_symlink_file_times(
            &target,
            filetime::FileTime::from_last_access_time(&metadata),
            filetime::FileTime::from_last_modification_time(&metadata),
        )?;
        Ok(())
    }

    fn permissions(&self, original: u32, directory: bool) -> Result<u32, Error> {
        let Some(specification) = self.mode else {
            return Ok(original);
        };
        if specification
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'7'))
            && let Ok(mode) = u32::from_str_radix(specification, 8)
            && mode <= 0o7777
        {
            return Ok(mode);
        }
        symbolic_mode(specification, original, directory)
    }
}

fn symbolic_mode(specification: &str, mut mode: u32, directory: bool) -> Result<u32, Error> {
    let invalid = || Error::Invalid(format!("invalid COPY mode {specification:?}"));
    for clause in specification.split(',') {
        let mut chars = clause.chars().peekable();
        let mut who = 0;
        while let Some(&ch) = chars.peek() {
            let bits = match ch {
                'u' => 0o4700,
                'g' => 0o2070,
                'o' => 0o0007,
                'a' => 0o6777,
                _ => break,
            };
            who |= bits;
            chars.next();
        }
        let mut operations = 0;
        while let Some(op) = chars.next() {
            if !matches!(op, '+' | '-' | '=') {
                return Err(invalid());
            }
            operations += 1;
            // Sticky participates in the affected mask only for an explicit
            // t token, and not for other-only clauses (BuildKit dchapes-mode).
            who &= !0o1000;
            let mut bits = 0;
            while let Some(&ch) = chars.peek() {
                if matches!(ch, '+' | '-' | '=') {
                    break;
                }
                chars.next();
                bits |= match ch {
                    'r' => 0o444,
                    'w' => 0o222,
                    'x' => 0o111,
                    'X' => {
                        if directory || mode & 0o111 != 0 {
                            0o111
                        } else {
                            0
                        }
                    }
                    's' => 0o6000,
                    't' => {
                        if who == 0 || who & !0o7 != 0 {
                            who |= 0o1000;
                            0o1000
                        } else {
                            0
                        }
                    }
                    'u' | 'g' | 'o' => {
                        let value = (mode
                            >> match ch {
                                'u' => 6,
                                'g' => 3,
                                _ => 0,
                            })
                            & 7;
                        value * 0o111
                    }
                    _ => return Err(invalid()),
                };
            }
            let affected = if who == 0 { 0o6777 } else { who };
            bits &= affected;
            match op {
                '+' => mode |= bits,
                '-' => mode &= !bits,
                '=' => mode = (mode & !affected) | bits,
                _ => unreachable!(),
            }
        }
        if operations == 0 {
            return Err(invalid());
        }
    }
    Ok(mode)
}

#[cfg(test)]
mod tests {
    use super::{Policy, ownership};
    use std::fs;

    #[test]
    fn modes_follow_buildkit_sticky_scoping_and_unsigned_octal() {
        for (spec, before, directory, expected) in [
            ("u+t", 0o644, false, 0o1644),
            ("g+t", 0o644, false, 0o1644),
            ("o+t", 0o644, false, 0o644),
            ("a+t", 0o644, false, 0o1644),
            ("+t", 0o644, false, 0o1644),
            // In the reference parser, an unqualified t sets who to sticky;
            // other permissions in that operation are then masked out.
            ("+rt", 0o000, false, 0o1000),
            ("+tr", 0o000, false, 0o1000),
            ("a+rt", 0o000, false, 0o1444),
            ("=rw", 0o1644, false, 0o1666),
            ("u-t", 0o1644, false, 0o644),
            ("o-t", 0o1644, false, 0o1644),
            ("a-t", 0o1644, false, 0o644),
            ("u=rw", 0o1644, false, 0o1644),
            ("o=rw", 0o1644, false, 0o1646),
            ("a=rw", 0o1644, false, 0o1666),
            ("u+t=rw", 0o644, false, 0o1644),
            ("a=rw-t", 0o1644, false, 0o666),
            ("u+s,g+s,o+s", 0o644, false, 0o6644),
            ("u=rw,go=r", 0o777, false, 0o644),
            ("g=u", 0o640, false, 0o660),
            ("a+X", 0o640, false, 0o640),
            ("a+X", 0o640, true, 0o751),
            ("0755", 0o1644, false, 0o755),
        ] {
            let policy = Policy {
                uid: 0,
                gid: 0,
                mode: Some(spec),
            };
            assert_eq!(
                policy.permissions(before, directory).unwrap(),
                expected,
                "{spec}"
            );
        }
        for spec in ["+7", "-7", "0788", "10000", "u+q", "u", ""] {
            let policy = Policy {
                uid: 0,
                gid: 0,
                mode: Some(spec),
            };
            assert!(policy.permissions(0o644, false).is_err(), "{spec}");
        }
    }

    #[test]
    fn named_owner_uses_passwd_primary_group_unless_overridden() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("etc")).unwrap();
        fs::write(
            root.path().join("etc/passwd"),
            "builder:x:1234:9876::/:/bin/sh\n",
        )
        .unwrap();
        // Deliberately no group named builder and no entry for its primary GID.
        fs::write(root.path().join("etc/group"), "builders:x:2345:\n").unwrap();
        for (owner, group, expected) in [
            (None, None, (0, 0)),
            (Some("1234"), None, (1234, 1234)),
            (Some("builder"), None, (1234, 9876)),
            (Some("builder"), Some("builders"), (1234, 2345)),
            (Some("builder"), Some("5"), (1234, 5)),
            (None, Some("builders"), (0, 2345)),
        ] {
            assert_eq!(ownership(root.path(), owner, group).unwrap(), expected);
        }
        assert!(ownership(root.path(), Some("missing"), None).is_err());
        assert!(ownership(root.path(), None, Some("missing")).is_err());
    }
}
