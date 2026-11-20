// SPDX-FileCopyrightText: Advanced Micro Devices, Inc.
// SPDX-License-Identifier: Apache-2.0

/// Shared serde predicate for values equal to their type's default.
pub(crate) trait IsDefault: Default + PartialEq {
    fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

impl<T: Default + PartialEq> IsDefault for T {}
