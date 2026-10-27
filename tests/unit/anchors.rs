// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use conglobate::anchor::RecoveryDirectory;

#[test]
fn ordinary_directory_cannot_become_a_recovery_lease() {
    let parent = tempfile::tempdir().unwrap();
    assert!(RecoveryDirectory::open(parent.path()).is_err());
    assert_eq!(std::fs::read_dir(parent.path()).unwrap().count(), 0);
}
