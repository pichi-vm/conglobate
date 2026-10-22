// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::chain::ScuteExtent;
use sha2::{Digest, Sha256};

#[test]
fn one_block_root_matches_salted_sha256_and_zero_padding() {
    let salt = [17; 32];
    let extent = ScuteExtent::new(5, &salt, Some(1)).unwrap();
    let root = extent.root(&b"hello"[..]).unwrap();
    let mut hash = Sha256::new();
    hash.update(salt);
    hash.update(b"hello");
    hash.update([0; 4091]);
    assert_eq!(root.as_slice(), hash.finalize().as_slice());
    assert_eq!(extent.parent(), &salt);
}

#[test]
fn extent_validation_and_exact_eof_are_required() {
    assert!(ScuteExtent::new(0, &[0; 32], None).is_err());
    assert!(ScuteExtent::new(1, &[0; 31], None).is_err());
    assert!(ScuteExtent::new(1, &[0; 32], Some(2)).is_err());
    assert!(ScuteExtent::new(u64::MAX, &[0; 32], None).is_err());
    let extent = ScuteExtent::new(5, &[0; 32], None).unwrap();
    assert!(extent.root(&b"four"[..]).is_err());
    assert!(extent.root(&b"sixsix"[..]).is_err());
}

#[test]
fn multiple_data_blocks_hash_through_a_padded_hash_block() {
    let salt = [3; 32];
    let input = vec![42; 8192];
    let mut leaf = Sha256::new();
    leaf.update(salt);
    leaf.update(&input[..4096]);
    let leaf = leaf.finalize();
    let mut root = Sha256::new();
    root.update(salt);
    root.update(leaf);
    root.update(leaf);
    root.update([0; 4032]);
    let actual = ScuteExtent::new(8192, &salt, Some(2))
        .unwrap()
        .root(input.as_slice())
        .unwrap();
    assert_eq!(actual.as_slice(), root.finalize().as_slice());
}

#[test]
fn emitted_tree_contains_independently_calculated_leaf_hashes() {
    let salt = [3; 32];
    let input = vec![42; 8192];
    let extent = ScuteExtent::new(8192, &salt, Some(2)).unwrap();
    let mut tree = std::io::Cursor::new(Vec::new());
    let root = extent.format(input.as_slice(), &mut tree).unwrap();
    let mut leaf = Sha256::new();
    leaf.update(salt);
    leaf.update(&input[..4096]);
    let leaf = leaf.finalize();
    let bytes = tree.into_inner();
    assert_eq!(bytes.len(), 8192);
    assert_eq!(&bytes[4096..4128], leaf.as_slice());
    assert_eq!(&bytes[4128..4160], leaf.as_slice());
    assert!(bytes[4160..].iter().all(|byte| *byte == 0));
    let mut expected = Sha256::new();
    expected.update(salt);
    expected.update(&bytes[4096..]);
    assert_eq!(root.as_slice(), expected.finalize().as_slice());

    let mut short_output = [0u8; 8];
    assert!(
        extent
            .format(
                input.as_slice(),
                std::io::Cursor::new(short_output.as_mut_slice())
            )
            .is_err()
    );
}

#[derive(Debug)]
struct LateError(std::io::Cursor<Vec<u8>>);
impl std::io::Read for LateError {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        if self.0.position() == self.0.get_ref().len() as u64 {
            Err(std::io::Error::other("fixture decoder trailer failure"))
        } else {
            std::io::Read::read(&mut self.0, bytes)
        }
    }
}
#[test]
fn decoder_errors_after_the_declared_extent_are_not_lost() {
    let reader = LateError(std::io::Cursor::new(vec![0; 4096]));
    let error = ScuteExtent::new(4096, &[0; 32], None)
        .unwrap()
        .root(reader)
        .unwrap_err();
    assert!(error.to_string().contains("trailer failure"));
}
