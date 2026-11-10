// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

//! Temporary entry point for images predating caller-supplied terminal settings.
mod mounts;

pub(crate) fn run() -> anyhow::Result<()> {
    let mappings = mounts::id_mappings();
    crate::terminal::run(crate::request::RuntimeArguments {
        repository: mounts::repository().to_path_buf(),
        cache: mounts::cache().to_path_buf(),
        output: mounts::output().to_path_buf(),
        uid_map: Some(mappings.uid()),
        gid_map: Some(mappings.gid()),
    })
}
