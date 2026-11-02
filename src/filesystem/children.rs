// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use rustix::fs::{self, AtFlags, Mode, OFlags, ResolveFlags, StatxFlags};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::File,
    io::{self, Read},
    os::{fd::AsRawFd, unix::ffi::OsStringExt},
    path::{Path, PathBuf},
};

/// Pins the locations of a supervisor-supplied mount tree's children before
/// admitting image code. Cleanup does not rediscover paths that code can rename.
#[derive(Debug, Default)]
pub(super) struct MountChildren {
    // Parents first. Pop children first during explicit, non-lazy unmount.
    points: Vec<(File, OsString)>,
}

impl MountChildren {
    pub(super) fn capture(root: &File) -> io::Result<Self> {
        let root_id = Self::identity(root, "")?;
        let table = File::open("/proc/thread-self/mountinfo")?;
        if fs::fstatfs(&table)?.f_type != 0x9fa0 {
            return Err(io::Error::other("mount tree capture requires procfs"));
        }
        let mut bytes = Vec::new();
        table.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > 2 * 1024 * 1024 {
            return Err(io::Error::other("mount table exceeds capture limit"));
        }
        let entries = Self::entries(&bytes)?;
        let (_, root_path) = entries
            .get(&root_id)
            .ok_or_else(|| io::Error::other("assigned mount is not attached"))?;
        let mut selected = BTreeSet::from([root_id]);
        loop {
            let before = selected.len();
            for (&id, (parent, _)) in &entries {
                if selected.contains(parent) {
                    selected.insert(id);
                }
            }
            if before == selected.len() {
                break;
            }
        }
        selected.remove(&root_id);
        let mut paths = selected
            .into_iter()
            .map(|id| {
                let path = entries[&id]
                    .1
                    .strip_prefix(root_path)
                    .map_err(io::Error::other)?;
                if path.as_os_str().is_empty() {
                    return Err(io::Error::other("overmounted assigned root"));
                }
                Ok((id, path))
            })
            .collect::<io::Result<Vec<_>>>()?;
        paths.sort_by_key(|(_, path)| path.components().count());
        let mut points = Vec::with_capacity(paths.len());
        for (id, path) in paths {
            let parent = path
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            let parent = File::from(fs::openat2(
                root,
                parent,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS,
            )?);
            let name = path
                .file_name()
                .ok_or_else(|| io::Error::other("mount has no name"))?;
            if Self::identity(&parent, name)? != id {
                return Err(io::Error::other(
                    "assigned child mount changed during capture",
                ));
            }
            points.push((parent, name.to_owned()));
        }
        Ok(Self { points })
    }

    pub(super) fn close(&mut self) -> io::Result<()> {
        while let Some((parent, name)) = self.points.last() {
            let path = PathBuf::from(format!("/proc/self/fd/{}", parent.as_raw_fd())).join(name);
            rustix::mount::unmount(path, rustix::mount::UnmountFlags::NOFOLLOW)?;
            self.points.pop();
        }
        Ok(())
    }

    fn identity(file: &File, path: impl rustix::path::Arg) -> io::Result<u64> {
        let stat = fs::statx(
            file,
            path,
            AtFlags::EMPTY_PATH | AtFlags::SYMLINK_NOFOLLOW,
            StatxFlags::MNT_ID,
        )?;
        if stat.stx_mask & StatxFlags::MNT_ID.bits() == 0 || stat.stx_mnt_id == 0 {
            return Err(io::Error::other("kernel did not report assigned mount ID"));
        }
        Ok(stat.stx_mnt_id)
    }

    fn entries(bytes: &[u8]) -> io::Result<BTreeMap<u64, (u64, PathBuf)>> {
        let mut entries = BTreeMap::new();
        for line in bytes
            .split(|&byte| byte == b'\n')
            .filter(|line| !line.is_empty())
        {
            let fields = line.split(|&byte| byte == b' ').take(6).collect::<Vec<_>>();
            if fields.len() != 6 {
                return Err(io::Error::other("incomplete kernel mount record"));
            }
            let id = std::str::from_utf8(fields[0])
                .map_err(io::Error::other)?
                .parse::<u64>()
                .map_err(io::Error::other)?;
            let parent = std::str::from_utf8(fields[1])
                .map_err(io::Error::other)?
                .parse::<u64>()
                .map_err(io::Error::other)?;
            let path = Self::path(fields[4])?;
            if id == 0 || !path.is_absolute() || entries.insert(id, (parent, path)).is_some() {
                return Err(io::Error::other("invalid kernel mount identity"));
            }
        }
        Ok(entries)
    }

    fn path(mut bytes: &[u8]) -> io::Result<PathBuf> {
        let mut result = Vec::new();
        while let Some((&first, rest)) = bytes.split_first() {
            if first == b'\\' {
                let (escaped, rest) = bytes
                    .split_at_checked(4)
                    .ok_or_else(|| io::Error::other("truncated mount path escape"))?;
                result.push(match escaped {
                    b"\\040" => b' ',
                    b"\\011" => b'\t',
                    b"\\012" => b'\n',
                    b"\\134" => b'\\',
                    _ => return Err(io::Error::other("invalid mount path escape")),
                });
                bytes = rest;
            } else {
                if first == 0 {
                    return Err(io::Error::other("NUL in kernel mount path"));
                }
                result.push(first);
                bytes = rest;
            }
        }
        Ok(PathBuf::from(OsString::from_vec(result)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_mount_paths_preserve_escaped_and_non_unicode_names() {
        assert_eq!(
            MountChildren::path(b"/root/a\\040b\\011c\\012d\\134e").unwrap(),
            Path::new("/root/a b\tc\nd\\e")
        );
        assert_eq!(
            MountChildren::path(b"/root/\xff").unwrap(),
            PathBuf::from(OsString::from_vec(b"/root/\xff".to_vec()))
        );
        for invalid in [b"/root/\\".as_slice(), b"/root/\\040\\999", b"/root/\0"] {
            assert!(MountChildren::path(invalid).is_err());
        }
        let table = b"10 1 0:1 / /root rw - tmpfs tmpfs rw\n11 10 0:2 / /root/a\\040b rw - tmpfs tmpfs rw\n";
        let entries = MountChildren::entries(table).unwrap();
        assert_eq!(entries[&11], (10, PathBuf::from("/root/a b")));
        assert!(MountChildren::entries(&[table.as_slice(), table.as_slice()].concat()).is_err());
    }
}
