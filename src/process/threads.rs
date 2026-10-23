// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

use std::{fs, io};

/// Diagnostic observation, never a proof of continued thread exclusivity.
/// Trusted helper startup must independently prevent concurrent thread creation.
#[derive(Debug)]
pub(crate) struct ThreadSnapshot {
    count: usize,
}
impl ThreadSnapshot {
    pub(crate) fn capture() -> io::Result<Self> {
        let count = fs::read_dir("/proc/self/task")?
            .try_fold(0, |count, entry| entry.map(|_| count + 1))?;
        Ok(Self { count })
    }

    pub(crate) fn is_single(&self) -> bool {
        self.count == 1
    }
}
