// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{fs, path::Path, process::Command};

pub(crate) fn install(root: &Path, binary: &str) {
    let destination = root.join(binary.trim_start_matches('/'));
    fs::create_dir_all(destination.parent().unwrap()).unwrap();
    fs::copy(binary, destination).unwrap();
    let dependencies = Command::new("ldd").arg(binary).output().unwrap();
    assert!(dependencies.status.success());
    for word in String::from_utf8(dependencies.stdout)
        .unwrap()
        .split_whitespace()
    {
        if !word.starts_with('/') {
            continue;
        }
        let destination = root.join(word.trim_start_matches('/'));
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(word, destination).unwrap();
    }
}
