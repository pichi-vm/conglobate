// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    planner::FilesystemSpec,
    value::{FilesystemLabel, FilesystemSize},
};
#[cfg(test)]
use sha2::{Digest, Sha256};
use std::{ffi::OsString, path::Path};

/// Immutable formatter invocation derived from validated plan properties.
/// This describes intended work; only the execution owner can issue a receipt.
#[derive(Debug)]
pub(crate) struct Ext4Format {
    size: FilesystemSize,
    label: FilesystemLabel,
}
impl From<&FilesystemSpec> for Ext4Format {
    fn from(spec: &FilesystemSpec) -> Self {
        Self::new(spec.size(), spec.label().clone())
    }
}
impl Ext4Format {
    pub(crate) const PROFILE: &'static str = "conglobate-ext4-v1";
    pub(crate) const CONFIGURATION_PATH: &'static str = "etc/mke2fs.conf";
    const OPTIONS: &'static [&'static str] = &[
        "-q",
        "-F",
        "-t",
        "ext4",
        "-T",
        "conglobate",
        "-b",
        "4096",
        "-I",
        "256",
        "-i",
        "16384",
        "-m",
        "0",
        "-E",
        "root_owner=0:0,nodiscard,lazy_itable_init=0",
    ];
    pub(crate) fn new(size: FilesystemSize, label: FilesystemLabel) -> Self {
        Self { size, label }
    }
    pub(crate) fn layout(&self) -> std::io::Result<crate::disk::Layout> {
        crate::disk::Layout::new(self.size.bytes())
    }
    #[cfg(test)]
    pub(crate) fn size(&self) -> FilesystemSize {
        self.size
    }
    pub(crate) fn configuration() -> &'static [u8] {
        include_bytes!("ext4.conf")
    }
    #[cfg(test)]
    pub(crate) fn configuration_digest() -> [u8; 32] {
        Sha256::digest(Self::configuration()).into()
    }
    /// The trusted execution owner supplies its device locator. Never use a
    /// plan path here. Each label is a single argument, not shell text.
    pub(crate) fn arguments(&self, device: &Path) -> Vec<OsString> {
        let mut args: Vec<_> = Self::OPTIONS.iter().map(OsString::from).collect();
        args.extend([
            "-L".into(),
            self.label.as_ref().into(),
            device.as_os_str().to_owned(),
            self.size.blocks().to_string().into(),
        ]);
        args
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the mke2fs executable"]
    fn new_ext4_root_has_no_journal() {
        use std::{fs::File, os::unix::fs::FileExt, process::Command};

        let directory = tempfile::tempdir().unwrap();
        let configuration = directory.path().join("mke2fs.conf");
        std::fs::write(&configuration, Ext4Format::configuration()).unwrap();
        let path = directory.path().join("root.ext4");
        File::create(&path).unwrap().set_len(64 << 20).unwrap();
        let format = Ext4Format::new("64MiB".parse().unwrap(), FilesystemLabel::default());
        assert!(
            Command::new("mke2fs")
                .env("MKE2FS_CONFIG", configuration)
                .args(format.arguments(&path))
                .status()
                .unwrap()
                .success()
        );
        let root = File::open(path).unwrap();
        let mut magic = [0; 2];
        root.read_exact_at(&mut magic, 1024 + 56).unwrap();
        assert_eq!(magic, [0x53, 0xef]);
        let mut compatible = [0; 4];
        root.read_exact_at(&mut compatible, 1024 + 92).unwrap();
        assert_eq!(u32::from_le_bytes(compatible) & 4, 0);
    }

    #[test]
    fn capacity_and_label_are_data_in_fixed_formatter_arguments() {
        let format = Ext4Format::new(
            "16MiB".parse().unwrap(),
            "-x; $(false)".to_owned().try_into().unwrap(),
        );
        let args = format.arguments(Path::new("/dev/output"));
        assert!(args.windows(2).any(|pair| pair == ["-T", "conglobate"]));
        assert_eq!(
            &args[args.len() - 4..],
            ["-L", "-x; $(false)", "/dev/output", "4096"].map(OsString::from)
        );
        assert_eq!(format.size().bytes(), 16 * 1024 * 1024);
        let larger = Ext4Format::new("65GiB".parse().unwrap(), FilesystemLabel::default());
        let args = larger.arguments(Path::new("/dev/output"));
        assert_eq!(args.last().unwrap(), "17039360");
    }
}
