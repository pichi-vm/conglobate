// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Open the named virtio console via sysfs virtio-ports. No udev symlink dependency.

use std::{fs, io, path::Path};

pub(crate) const CONSOLE_NAME: &str = "dev.pichi.build";

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConsoleError {
    #[error("console I/O: {0}")]
    Io(#[from] io::Error),
    #[error("build console {CONSOLE_NAME} was not found under sysfs virtio-ports")]
    NotFound,
}

/// Locate `<dev_root>/<port>` for the port whose sysfs `name` equals `dev.pichi.build`.
///
/// `sysfs_root` is typically `/sys` and `dev_root` typically `/dev`. Tests inject
/// temporary trees for both so the success path (open + oneshot read) is covered
/// without a real virtio console node.
pub(crate) fn open_build_console(
    sysfs_root: &Path,
    dev_root: &Path,
) -> Result<fs::File, ConsoleError> {
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
        let device = dev_root.join(dev_name);
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
    fn opens_named_console_from_injected_sysfs_and_dev() {
        let root = tempfile::TempDir::new().unwrap();
        let sysfs = root.path().join("sys");
        let dev = root.path().join("dev");
        let ports = sysfs.join("class/virtio-ports");
        fs::create_dir_all(ports.join("vport0p1")).unwrap();
        fs::create_dir_all(ports.join("vport0p2")).unwrap();
        fs::create_dir_all(&dev).unwrap();
        fs::write(ports.join("vport0p1/name"), "other\n").unwrap();
        fs::write(ports.join("vport0p2/name"), "dev.pichi.build\n").unwrap();
        let payload = b"\xa1\x63ref\x61x";
        fs::write(dev.join("vport0p2"), payload).unwrap();

        let file = open_build_console(&sysfs, &dev).unwrap();
        let bytes = read_oneshot(file).unwrap();
        assert_eq!(bytes, payload);
    }

    #[test]
    fn finds_named_port_but_missing_device_is_io_error() {
        let root = tempfile::TempDir::new().unwrap();
        let sysfs = root.path().join("sys");
        let dev = root.path().join("dev");
        let ports = sysfs.join("class/virtio-ports");
        fs::create_dir_all(ports.join("vport0p2")).unwrap();
        fs::create_dir_all(&dev).unwrap();
        fs::write(ports.join("vport0p2/name"), "dev.pichi.build\n").unwrap();
        // Name matches, but no device node under the injected /dev.
        let error = open_build_console(&sysfs, &dev).unwrap_err().to_string();
        assert!(
            error.contains("No such file") || error.contains("os error"),
            "{error}"
        );
    }

    #[test]
    fn missing_ports_directory_is_not_found() {
        let root = tempfile::TempDir::new().unwrap();
        let error = open_build_console(root.path(), Path::new("/dev")).unwrap_err();
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
