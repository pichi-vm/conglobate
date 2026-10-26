// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Open the named virtio console via sysfs/hvc. No udev symlink dependency.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(crate) const CONSOLE_NAME: &str = "dev.pichi.build";

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConsoleError {
    #[error("console I/O: {0}")]
    Io(#[from] io::Error),
    #[error("build console {CONSOLE_NAME} was not found under sysfs virtio-ports")]
    NotFound,
}

/// Locate `/dev/<port>` for the port whose sysfs `name` equals `dev.pichi.build`.
/// `sysfs_root` is typically `/sys`; tests inject a temporary tree.
pub(crate) fn open_build_console(sysfs_root: &Path) -> Result<fs::File, ConsoleError> {
    let ports = sysfs_root.join("class/virtio-ports");
    let mut entries = fs::read_dir(&ports).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ConsoleError::NotFound
        } else {
            ConsoleError::Io(error)
        }
    })?;
    while let Some(entry) = entries.next().transpose()? {
        let path = entry.path();
        let name_path = path.join("name");
        let Ok(name) = fs::read_to_string(&name_path) else {
            continue;
        };
        let name = name.trim_end_matches(['\n', '\r']);
        if name != CONSOLE_NAME {
            continue;
        }
        let dev_name = path.file_name().ok_or(ConsoleError::NotFound)?;
        let device = PathBuf::from("/dev").join(dev_name);
        return Ok(fs::File::open(device)?);
    }
    Err(ConsoleError::NotFound)
}

/// One-shot read of the console FD to EOF. Caller closes by dropping the file.
pub(crate) fn read_oneshot(mut file: fs::File) -> Result<Vec<u8>, ConsoleError> {
    use std::io::Read;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finds_named_port_under_injected_sysfs() {
        let root = tempfile::TempDir::new().unwrap();
        let ports = root.path().join("class/virtio-ports");
        fs::create_dir_all(ports.join("vport0p1")).unwrap();
        fs::create_dir_all(ports.join("vport0p2")).unwrap();
        fs::write(ports.join("vport0p1/name"), "other\n").unwrap();
        fs::write(ports.join("vport0p2/name"), "dev.pichi.build\n").unwrap();
        // Point /dev/vport0p2 at a temp file via a private "dev" is not possible
        // without root; open_build_console opens /dev/<name>. Exercise discovery
        // failure path when the device node is absent, after name match.
        let error = open_build_console(root.path()).unwrap_err().to_string();
        assert!(
            error.contains("No such file") || error.contains("os error"),
            "{error}"
        );
    }

    #[test]
    fn missing_ports_directory_is_not_found() {
        let root = tempfile::TempDir::new().unwrap();
        let error = open_build_console(root.path()).unwrap_err();
        assert!(matches!(error, ConsoleError::NotFound), "{error:?}");
    }

    #[test]
    fn oneshot_reads_entire_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("console");
        fs::write(&path, b"\xa1\x63ref\x61x").unwrap();
        let bytes = read_oneshot(fs::File::open(path).unwrap()).unwrap();
        assert_eq!(bytes, b"\xa1\x63ref\x61x");
    }
}
