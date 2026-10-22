// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::policy::{FilesystemTarget, LabelScope, ScopeError};

#[test]
fn pin_directory_rejects_non_bpffs_without_creating_entries() {
    let parent = tempfile::tempdir().unwrap();
    assert!(conglobate::policy::PinDirectory::create(parent.path()).is_err());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}

#[test]
fn embedded_object_matches_the_map_and_hook_contract_without_loading() {
    let target = FilesystemTarget::new(42, rustix::fs::makedev(7, 1)).unwrap();
    let scope = LabelScope::new(99, 100000, [target]).unwrap();
    let prepared = conglobate::policy::LabelPolicy::new(scope).unwrap();
    assert_eq!(
        prepared.scope().targets().copied().collect::<Vec<_>>(),
        vec![target]
    );
}

#[test]
fn map_encoding_separates_subject_from_superblock_identity() {
    let target = FilesystemTarget::new(42, rustix::fs::makedev(7, 256)).unwrap();
    let scope = LabelScope::new(99, 100000, [target]).unwrap();
    assert_eq!(&scope.subject_value()[..4], &99u32.to_ne_bytes());
    assert_eq!(&scope.subject_value()[4..], &100000u32.to_ne_bytes());
    assert_eq!(&target.map_key()[..4], &42u32.to_ne_bytes());
    assert_eq!(&target.map_key()[4..], &((7u32 << 20) | 256).to_ne_bytes());
    assert_eq!(target.map_value(), 1u32.to_ne_bytes());
}

#[test]
fn target_set_is_explicit_bounded_and_independent_of_input_order() {
    let a = FilesystemTarget::new(42, rustix::fs::makedev(7, 1)).unwrap();
    let b = FilesystemTarget::new(42, rustix::fs::makedev(7, 2)).unwrap();
    let c = FilesystemTarget::new(42, rustix::fs::makedev(7, 3)).unwrap();
    let scope = LabelScope::new(99, 100000, [b, a]).unwrap();
    assert_eq!(scope.targets().copied().collect::<Vec<_>>(), vec![a, b]);
    assert!(matches!(
        LabelScope::new(99, 100000, []),
        Err(ScopeError::TargetCount)
    ));
    assert!(matches!(
        LabelScope::new(99, 100000, [a, a]),
        Err(ScopeError::Duplicate)
    ));
    assert!(matches!(
        LabelScope::new(99, 100000, [a, b, c]),
        Err(ScopeError::TargetCount)
    ));
    assert!(matches!(
        LabelScope::new(42, 100000, [a]),
        Err(ScopeError::SuperblockOwner)
    ));
}

#[test]
fn reject_identity_truncation_and_host_root_mapping() {
    for ns in [0, u32::MAX as u64 + 1] {
        assert!(matches!(
            FilesystemTarget::new(ns, 0),
            Err(ScopeError::Namespace)
        ));
        let target = FilesystemTarget::new(42, 0).unwrap();
        assert!(matches!(
            LabelScope::new(ns, 100000, [target]),
            Err(ScopeError::Namespace)
        ));
    }
    assert!(matches!(
        FilesystemTarget::new(42, rustix::fs::makedev(4096, 0)),
        Err(ScopeError::Device)
    ));
    assert!(matches!(
        FilesystemTarget::new(42, rustix::fs::makedev(0, 1 << 20)),
        Err(ScopeError::Device)
    ));
    let target = FilesystemTarget::new(42, rustix::fs::makedev(4095, (1 << 20) - 1)).unwrap();
    assert_eq!(&target.map_key()[4..], &u32::MAX.to_ne_bytes());
    assert!(matches!(
        LabelScope::new(99, 0, [target]),
        Err(ScopeError::Root)
    ));
    assert!(matches!(
        LabelScope::new(99, u32::MAX, [target]),
        Err(ScopeError::Root)
    ));
}
