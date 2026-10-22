// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::domain::DomainRecord;
#[derive(Debug)]
struct Fixture;
impl Fixture {
    fn bytes(name: &str, parent_inode: u64, inode: u64) -> Vec<u8> {
        let mut e = minicbor::Encoder::new(Vec::new());
        e.array(6)
            .unwrap()
            .u8(1)
            .unwrap()
            .str(name)
            .unwrap()
            .u64(1)
            .unwrap()
            .u64(parent_inode)
            .unwrap()
            .u64(3)
            .unwrap()
            .u64(inode)
            .unwrap();
        e.into_writer()
    }
}
#[test]
fn canonical_domain_description_roundtrips() {
    let bytes = Fixture::bytes("conglobate-command-a", 2, 4);
    assert_eq!(bytes, b"\x86\x01\x74conglobate-command-a\x01\x02\x03\x04");
    let record = DomainRecord::try_from(bytes.as_slice()).unwrap();
    assert_eq!(record.name(), "conglobate-command-a");
    assert_eq!(record.to_cbor(), bytes);
}
#[test]
fn reject_invalid_names_and_zero_identity_inodes() {
    for name in [
        "",
        "conglobate-command-",
        "conglobate-command-../x",
        "/conglobate-command-a",
        "conglobate-command-é",
        "namespace-a",
    ] {
        assert!(DomainRecord::try_from(Fixture::bytes(name, 2, 4).as_slice()).is_err());
    }
    for (parent, inode) in [(0, 4), (2, 0)] {
        assert!(
            DomainRecord::try_from(
                Fixture::bytes("conglobate-command-a", parent, inode).as_slice()
            )
            .is_err()
        );
    }
}
#[test]
fn reject_truncated_noncanonical_oversized_and_trailing_records() {
    let bytes = Fixture::bytes("conglobate-command-a", 2, 4);
    for end in 0..bytes.len() {
        assert!(DomainRecord::try_from(&bytes[..end]).is_err());
    }
    let mut wider = vec![0x98, 6];
    wider.extend_from_slice(&bytes[1..]);
    assert!(DomainRecord::try_from(wider.as_slice()).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(DomainRecord::try_from(trailing.as_slice()).is_err());
    assert!(DomainRecord::try_from(&[0u8; 257][..]).is_err());
    let mut version = bytes;
    version[1] = 2;
    assert!(DomainRecord::try_from(version.as_slice()).is_err());
}
