// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Open the named virtio console via sysfs virtio-ports. No udev symlink dependency.
//!
//! v0 build-port: guest opens `dev.pichi.build` read/write, writes one CBOR
//! request map, then reads one self-delimiting CBOR response. The guest must
//! not assume the host already pushed bytes before the request.

use ciborium::Value;
use std::{
    fs,
    io::{self, Write},
    path::Path,
};

pub(crate) const CONSOLE_NAME: &str = "dev.pichi.build";
pub(crate) const PROTOCOL_VERSION: i64 = 1;
pub(crate) const BUILD_INPUT_OP: &str = "build-input";

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConsoleError {
    #[error("console I/O: {0}")]
    Io(#[from] io::Error),
    #[error("build console {CONSOLE_NAME} was not found under sysfs virtio-ports")]
    NotFound,
    #[error("build-port request encode: {0}")]
    Encode(String),
}

/// Locate `<dev_root>/<port>` for the port whose sysfs `name` equals `dev.pichi.build`.
///
/// Opens the device read/write. `sysfs_root` is typically `/sys` and `dev_root`
/// typically `/dev`. Tests inject temporary trees for both.
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
        return Ok(fs::OpenOptions::new().read(true).write(true).open(device)?);
    }
    Err(ConsoleError::NotFound)
}

/// Encode the v0 guest→host request: `{"v":1,"op":"build-input"}`.
pub(crate) fn encode_build_input_request() -> Result<Vec<u8>, ConsoleError> {
    let value = Value::Map(vec![
        (
            Value::Text("v".into()),
            Value::Integer(PROTOCOL_VERSION.into()),
        ),
        (Value::Text("op".into()), Value::Text(BUILD_INPUT_OP.into())),
    ]);
    let mut bytes = Vec::new();
    ciborium::into_writer(&value, &mut bytes)
        .map_err(|error| ConsoleError::Encode(error.to_string()))?;
    Ok(bytes)
}

/// Write the build-input request to the console (guest → host).
pub(crate) fn write_build_input_request(file: &mut impl Write) -> Result<(), ConsoleError> {
    let bytes = encode_build_input_request()?;
    file.write_all(&bytes)?;
    file.flush()?;
    Ok(())
}

/// Read one self-delimiting CBOR value from the console (host → guest).
///
/// Does not read to EOF: the host may leave the port open after the response.
/// Test helper: production reads via [`super::request::ApplianceRequest::decode_response`].
#[cfg(test)]
pub(crate) fn read_cbor_value(file: &mut impl std::io::Read) -> Result<Value, ConsoleError> {
    ciborium::from_reader(file)
        .map_err(|error| ConsoleError::Io(io::Error::other(error.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::Cursor,
        os::unix::net::UnixStream,
        sync::{Arc, Barrier},
        thread,
    };

    fn fixture_named_port(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
        let sysfs = root.join("sys");
        let dev = root.join("dev");
        let ports = sysfs.join("class/virtio-ports");
        fs::create_dir_all(ports.join("vport0p1")).unwrap();
        fs::create_dir_all(ports.join("vport0p2")).unwrap();
        fs::create_dir_all(&dev).unwrap();
        fs::write(ports.join("vport0p1/name"), "other\n").unwrap();
        fs::write(ports.join("vport0p2/name"), "dev.pichi.build\n").unwrap();
        (sysfs, dev)
    }

    #[test]
    fn encode_request_is_v1_build_input() {
        let bytes = encode_build_input_request().unwrap();
        let value: Value = ciborium::from_reader(Cursor::new(&bytes)).unwrap();
        let Value::Map(entries) = value else {
            panic!("expected map");
        };
        let mut map = std::collections::BTreeMap::new();
        for (k, v) in entries {
            let Value::Text(key) = k else {
                panic!("key");
            };
            map.insert(key, v);
        }
        assert_eq!(map.get("v"), Some(&Value::Integer(1.into())));
        assert_eq!(map.get("op"), Some(&Value::Text("build-input".into())));
    }

    #[test]
    fn opens_named_console_read_write() {
        let root = tempfile::TempDir::new().unwrap();
        let (sysfs, dev) = fixture_named_port(root.path());
        // Regular file stands in for the device node; open must succeed RDWR.
        fs::write(dev.join("vport0p2"), []).unwrap();
        let mut file = open_build_console(&sysfs, &dev).unwrap();
        write_build_input_request(&mut file).unwrap();
        let written = fs::read(dev.join("vport0p2")).unwrap();
        assert_eq!(written, encode_build_input_request().unwrap());
    }

    #[test]
    fn exchange_writes_request_then_reads_response() {
        let (mut guest, mut host) = UnixStream::pair().unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let host_barrier = Arc::clone(&barrier);
        let host_thread = thread::spawn(move || {
            host_barrier.wait();
            let request = read_cbor_value(&mut host).unwrap();
            let Value::Map(entries) = request else {
                panic!("request map");
            };
            assert!(entries.iter().any(|(k, v)| {
                matches!(k, Value::Text(t) if t == "op")
                    && matches!(v, Value::Text(t) if t == "build-input")
            }));
            let response = Value::Map(vec![
                (Value::Text("v".into()), Value::Integer(1.into())),
                (Value::Text("ok".into()), Value::Bool(true)),
                (
                    Value::Text("ref".into()),
                    Value::Text("refs/heads/main".into()),
                ),
                (
                    Value::Text("recipe".into()),
                    Value::Text("recipe.yml".into()),
                ),
                (Value::Text("lock".into()), Value::Text("lock.yml".into())),
            ]);
            ciborium::into_writer(&response, &mut host).unwrap();
        });
        barrier.wait();
        write_build_input_request(&mut guest).unwrap();
        let response = read_cbor_value(&mut guest).unwrap();
        host_thread.join().unwrap();
        let Value::Map(entries) = response else {
            panic!("response map");
        };
        assert!(entries.iter().any(|(k, v)| {
            matches!(k, Value::Text(t) if t == "ok") && matches!(v, Value::Bool(true))
        }));
    }

    #[test]
    fn finds_named_port_but_missing_device_is_io_error() {
        let root = tempfile::TempDir::new().unwrap();
        let (sysfs, dev) = fixture_named_port(root.path());
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
}
