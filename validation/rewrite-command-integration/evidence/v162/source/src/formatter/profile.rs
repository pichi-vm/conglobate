// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use crate::{
    recipe::FilesystemSpec,
    value::{FilesystemLabel, FilesystemSize},
};
use sha2::{Digest, Sha256};
use std::{ffi::OsString, path::Path};

/// Immutable formatter invocation derived from validated recipe properties.
/// This describes intended work; only the execution owner can issue a receipt.
#[derive(Debug)]
pub struct Ext4Format {
    size: FilesystemSize,
    label: FilesystemLabel,
}
impl From<&FilesystemSpec> for Ext4Format {
    fn from(spec: &FilesystemSpec) -> Self {
        Self::new(spec.size(), spec.label().clone())
    }
}
impl Ext4Format {
    pub const PROFILE: &'static str = "conglobate-ext4-v1";
    pub const CONFIGURATION_PATH: &'static str = "etc/mke2fs.conf";
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
        "root_owner=0:0,nodiscard,lazy_itable_init=0,lazy_journal_init=0",
    ];
    pub fn new(size: FilesystemSize, label: FilesystemLabel) -> Self {
        Self { size, label }
    }
    pub fn size(&self) -> FilesystemSize {
        self.size
    }
    pub fn label(&self) -> &FilesystemLabel {
        &self.label
    }
    pub fn configuration() -> &'static [u8] {
        include_bytes!("ext4.conf")
    }
    pub fn configuration_digest() -> [u8; 32] {
        Sha256::digest(Self::configuration()).into()
    }
    /// Versioned deterministic CBOR captures configuration and fixed arguments;
    /// recipe-specific label/capacity and the device locator are recorded separately.
    pub fn profile_bytes() -> Vec<u8> {
        let mut e = minicbor::Encoder::new(Vec::new());
        e.array(4)
            .expect("Vec writer")
            .u8(1)
            .expect("Vec writer")
            .str(Self::PROFILE)
            .expect("Vec writer")
            .bytes(Self::configuration())
            .expect("Vec writer")
            .array(Self::OPTIONS.len() as u64)
            .expect("Vec writer");
        for option in Self::OPTIONS {
            e.str(option).expect("Vec writer");
        }
        e.into_writer()
    }
    pub fn profile_digest() -> [u8; 32] {
        Sha256::digest(Self::profile_bytes()).into()
    }
    /// The trusted execution owner supplies its device locator. Never use a
    /// recipe path here. Each label is a single argument, not shell text.
    pub fn arguments(&self, device: &Path) -> Vec<OsString> {
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
    fn capacity_and_label_are_data_and_profile_identity_is_recipe_independent() {
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
        assert_eq!(
            FilesystemSize::LOGICAL_DEVICE_BYTES,
            64 * 1024 * 1024 * 1024
        );
        let bytes = Ext4Format::profile_bytes();
        let mut d = minicbor::Decoder::new(&bytes);
        assert_eq!(d.array().unwrap(), Some(4));
        assert_eq!(d.u8().unwrap(), 1);
        assert_eq!(d.str().unwrap(), Ext4Format::PROFILE);
        assert_eq!(d.bytes().unwrap(), Ext4Format::configuration());
        assert_eq!(
            Ext4Format::profile_digest(),
            <[u8; 32]>::from(Sha256::digest(bytes))
        );
    }
}
