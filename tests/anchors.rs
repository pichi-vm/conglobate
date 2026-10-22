// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0
use conglobate::anchor::{CreateFailure, NamespacePin};

#[test]
fn ordinary_file_is_rejected_before_any_mount_or_target_creation() {
    let parent = tempfile::tempdir().unwrap();
    let file = tempfile::tempfile().unwrap();
    assert!(matches!(
        NamespacePin::create(parent.path(), file),
        Err(CreateFailure::Before(_))
    ));
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}
