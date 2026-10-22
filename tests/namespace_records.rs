// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::anchor::NamespaceRecord;

#[derive(Debug)]
struct Fixture;
impl Fixture {
    fn bytes(version: u8, name: &str, inode: u64) -> Vec<u8> {
        let mut e = minicbor::Encoder::new(Vec::new());
        e.array(4)
            .unwrap()
            .u8(version)
            .unwrap()
            .str(name)
            .unwrap()
            .u64(4)
            .unwrap()
            .u64(inode)
            .unwrap();
        e.into_writer()
    }
}
#[test]
fn canonical_record_roundtrips_as_data() {
    let bytes = Fixture::bytes(1, "namespace-a", 24);
    assert_eq!(bytes, b"\x84\x01\x6bnamespace-a\x04\x18\x18");
    let record = NamespaceRecord::try_from(bytes.as_slice()).unwrap();
    assert_eq!(record.name(), "namespace-a");
    assert_eq!(record.inode(), 24);
    assert_eq!(record.to_cbor(), bytes);
}
#[test]
fn reject_unsafe_names_invalid_ids_and_unknown_versions() {
    for name in [
        "",
        ".",
        "..",
        "namespace-",
        "namespace-../x",
        "/namespace-a",
        "namespace-a/b",
        "namespace-a\0",
        "namespace-é",
    ] {
        assert!(NamespaceRecord::try_from(Fixture::bytes(1, name, 24).as_slice()).is_err());
    }
    for inode in [0, u32::MAX as u64 + 1] {
        assert!(
            NamespaceRecord::try_from(Fixture::bytes(1, "namespace-a", inode).as_slice()).is_err()
        );
    }
    assert!(NamespaceRecord::try_from(Fixture::bytes(2, "namespace-a", 24).as_slice()).is_err());
}
#[test]
fn reject_trailing_truncated_overlong_and_noncanonical_encodings() {
    let bytes = Fixture::bytes(1, "namespace-a", 24);
    for end in 0..bytes.len() {
        assert!(NamespaceRecord::try_from(&bytes[..end]).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(NamespaceRecord::try_from(trailing.as_slice()).is_err());
    let mut wider = vec![0x98, 4];
    wider.extend_from_slice(&bytes[1..]);
    assert!(NamespaceRecord::try_from(wider.as_slice()).is_err());
    let mut indefinite = vec![0x9f];
    indefinite.extend_from_slice(&bytes[1..]);
    indefinite.push(0xff);
    assert!(NamespaceRecord::try_from(indefinite.as_slice()).is_err());
    assert!(NamespaceRecord::try_from(&[0u8; 257][..]).is_err());
}
